// SPDX-License-Identifier: GPL-3.0-or-later

//! Deterministic, equation-free lowering from managed code projects into the
//! ordinary design-intent patch vocabulary.

use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch_intent::{
    AggregateKind, ComputedFeatureKind, ConstraintKind, GeometryRecipeKind, InputRole, InputSlot,
    IntentFieldKey, IntentKey, IntentKeyError, IntentLiteral, IntentNodeDraft, IntentNodeKind,
    IntentPatch, IntentPatchOperation, IntentPatchPolicy, IntentPortKind, IntentPortRole,
    IntentPortSelector, IntentSessionIdentity, IntentUnit, LeafField, PatchPortRef,
    intent_content_digest,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    ArtifactValidationError, AuthoringDeclaration, CodeDraftProvenance, CodeDraftValue,
    CodeGeneratedChildAddress, CodeInteractionOverlay, CodeOverlayError, CodeOwnerAddress,
    CodeProject, CodeProjectError, CodeWritableAddress, CollectionRule, DirectDeclarationLowering,
    FeatureKind, GeneratedMemberAddress, GeneratedMemberIdentity, KeyedReconcileError,
    KeyedReconcileState, ManagedPathSegment, ManagedValue, OutputRef, PatchModuleArtifact,
    PatchTemplateNode, ProjectKey, SemanticOutputPath, SemanticSymbol, TemplateBinding,
    TemplateDeclarationLowering, UnitLiteral, ValidatedPatchModuleArtifact,
    code_declaration_family,
};

const MAX_EXPANDED_NODES: usize = 65_536;
const MAX_EXPANDED_OUTPUTS: usize = 65_536;
const MAX_HOST_REQUESTS: usize = 65_536;

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
    pub digest: String,
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
}

#[derive(Clone, Debug)]
struct SemanticDeclaration {
    root: SemanticValue,
    paths: BTreeMap<SemanticOutputPath, SemanticValue>,
}

type CollectionPlan = BTreeMap<Vec<String>, Vec<(String, SemanticValue)>>;
type TemplateOutputKey = (Vec<String>, Vec<String>, String);
type GeneratedAddressPlan = BTreeMap<TemplateOutputKey, GeneratedMemberAddress>;
type TemplateOutputPlan = BTreeMap<TemplateOutputKey, SemanticValue>;
type SemanticPathMap = BTreeMap<SemanticOutputPath, SemanticValue>;
type SemanticMemberMap = BTreeMap<String, SemanticValue>;
type InvocationCollectionPublication = (SemanticPathMap, SemanticMemberMap);

#[derive(Clone, Debug)]
enum SemanticValue {
    Declaration {
        alias: IntentKey,
        kind: FeatureKind,
    },
    Port(ExpandedPort),
    Corner(ExpandedFeatureCorner),
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
    operations: Vec<IntentPatchOperation>,
    declarations: BTreeMap<SemanticSymbol, SemanticDeclaration>,
    provenance: BTreeMap<GeneratedMemberAddress, GeneratedIntentProvenance>,
    declaration_provenance: BTreeMap<IntentKey, SemanticSymbol>,
    writable_points: Vec<ExpandedWritablePoint>,
    generated_children: Vec<ExpandedGeneratedChild>,
    host_requests: Vec<CodeHostRequest>,
    point_seeds: BTreeMap<(IntentKey, IntentPortSelector), [f64; 2]>,
}

#[derive(Clone, Debug)]
struct KeyedPolylineDefinition {
    vertices: Vec<(String, [f64; 2])>,
    closed: bool,
}

impl ExpansionBuilder {
    fn new(project: ProjectKey) -> Self {
        Self {
            project,
            operations: Vec::new(),
            declarations: BTreeMap::new(),
            provenance: BTreeMap::new(),
            declaration_provenance: BTreeMap::new(),
            writable_points: Vec::new(),
            generated_children: Vec::new(),
            host_requests: Vec::new(),
            point_seeds: BTreeMap::new(),
        }
    }

    fn push_node(
        &mut self,
        declaration: &SemanticSymbol,
        alias: IntentKey,
        draft: IntentNodeDraft,
    ) -> Result<(), CodeExpansionError> {
        if self.operations.len() >= MAX_EXPANDED_NODES {
            return Err(CodeExpansionError::ResourceLimit {
                resource: "intent nodes",
                actual: self.operations.len().saturating_add(1),
                limit: MAX_EXPANDED_NODES,
            });
        }
        if let Some(previous) = self
            .declaration_provenance
            .insert(alias.clone(), declaration.clone())
        {
            return Err(CodeExpansionError::InvalidDeclaration {
                declaration: declaration.0.clone(),
                message: format!(
                    "expanded alias is already owned by declaration `{}`",
                    previous.0
                ),
            });
        }
        self.operations.push(IntentPatchOperation::CreateNode {
            alias,
            draft: Box::new(draft),
            cell: None,
        });
        Ok(())
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
    let (builder, plans) = prepare(project, None, &CodeInteractionOverlay::empty())?;
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
        match declaration.builder_path.as_slice() {
            [geometry, family]
                if geometry == "geometry" && matches!(family.as_str(), "line" | "rectangle") =>
            {
                addresses.push(direct_declaration_owner_address(
                    &declaration.symbol,
                    family,
                ));
                continue;
            }
            [geometry, family] if geometry == "geometry" && family == "polyline" => {}
            _ => continue,
        }
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
pub fn expand_code_project_with_overlay(
    project: &CodeProject,
    reconciliation: &KeyedReconcileState,
    overlay: &CodeInteractionOverlay,
    expected: IntentSessionIdentity,
) -> Result<ExpandedCodeProject, CodeExpansionError> {
    reconciliation.validate()?;
    overlay.validate()?;
    validate_supported_overrides(reconciliation)?;
    let (mut builder, plans) = prepare(project, Some(reconciliation), overlay)?;
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
        lower_invocation(&mut builder, &plan, reconciliation, overlay)?;
    }
    lower_computed(project, &mut builder)?;
    lower_constraints(project, &mut builder)?;

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
                request.suppressed = overlay
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
                    let suppressed = overlay
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
) -> Result<(ExpansionBuilder, Vec<InvocationPlan>), CodeExpansionError> {
    project.validate()?;
    let artifacts = pinned_artifacts(project)?;
    let mut builder = ExpansionBuilder::new(project.project.clone());

    for declaration in &project.managed.program.declarations {
        if declaration.patch.is_none()
            && declaration.builder_path.first().map(String::as_str) == Some("geometry")
        {
            lower_direct_geometry(&mut builder, declaration, reconciliation, overlay)?;
        }
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

fn lower_direct_geometry(
    builder: &mut ExpansionBuilder,
    declaration: &AuthoringDeclaration,
    reconciliation: Option<&KeyedReconcileState>,
    overlay: &CodeInteractionOverlay,
) -> Result<(), CodeExpansionError> {
    let family = declaration.builder_path.join(".");
    match code_declaration_family(&family).and_then(|descriptor| descriptor.direct) {
        Some(DirectDeclarationLowering::Line) => {
            lower_direct_line(builder, declaration, reconciliation, overlay)
        }
        Some(DirectDeclarationLowering::Polyline) => {
            lower_direct_polyline(builder, declaration, reconciliation, overlay)
        }
        Some(DirectDeclarationLowering::Rectangle) => {
            lower_direct_rectangle(builder, declaration, reconciliation, overlay)
        }
        Some(DirectDeclarationLowering::FilletSet) => Err(CodeExpansionError::Unsupported(
            format!("managed computed family `{family}` used through geometry"),
        )),
        Some(DirectDeclarationLowering::Constraint(_)) => Err(CodeExpansionError::Unsupported(
            format!("managed constraint family `{family}` used through geometry"),
        )),
        None => Err(CodeExpansionError::Unsupported(format!(
            "managed geometry family `{family}`"
        ))),
    }
}

fn lower_direct_computed(
    builder: &mut ExpansionBuilder,
    declaration: &AuthoringDeclaration,
) -> Result<(), CodeExpansionError> {
    let family = declaration.builder_path.join(".");
    match code_declaration_family(&family).and_then(|descriptor| descriptor.direct) {
        Some(DirectDeclarationLowering::FilletSet) => lower_direct_fillet_set(builder, declaration),
        Some(
            DirectDeclarationLowering::Line
            | DirectDeclarationLowering::Polyline
            | DirectDeclarationLowering::Rectangle
            | DirectDeclarationLowering::Constraint(_),
        ) => Err(CodeExpansionError::Unsupported(format!(
            "managed geometry family `{family}` used through computed"
        ))),
        None => Err(CodeExpansionError::Unsupported(format!(
            "managed computed family `{family}`"
        ))),
    }
}

fn lower_computed(
    project: &CodeProject,
    builder: &mut ExpansionBuilder,
) -> Result<(), CodeExpansionError> {
    for declaration in &project.managed.program.declarations {
        if declaration.patch.is_none()
            && declaration.builder_path.first().map(String::as_str) == Some("computed")
        {
            lower_direct_computed(builder, declaration)?;
        }
    }
    Ok(())
}

#[allow(
    clippy::too_many_lines,
    reason = "branch-explicit FilletSet lowering keeps every persisted contact field visibly adjacent"
)]
fn lower_direct_fillet_set(
    builder: &mut ExpansionBuilder,
    declaration: &AuthoringDeclaration,
) -> Result<(), CodeExpansionError> {
    let arguments = object(&declaration.arguments, &declaration.symbol.0)?;
    exact_object_keys(
        arguments,
        &["corners", "radius", "suppressed"],
        &declaration.symbol.0,
    )?;
    let radius = length_value(
        required(arguments, "radius", &declaration.symbol.0)?,
        "FilletSet radius",
    )?;
    if !radius.is_finite() || radius <= 0.0 {
        return invalid_declaration(
            declaration,
            "FilletSet radius must be finite and positive".into(),
        );
    }
    let corners = array(
        required(arguments, "corners", &declaration.symbol.0)?,
        "FilletSet corners",
    )?;
    let kind = IntentNodeKind::ComputedFeature {
        feature: ComputedFeatureKind::FilletSet,
    };
    let maximum_corners = usize::from(kind.schema(0).maximum_children);
    if corners.len() > maximum_corners {
        return Err(CodeExpansionError::ResourceLimit {
            resource: "direct FilletSet corners",
            actual: corners.len(),
            limit: maximum_corners,
        });
    }
    let child_count =
        u16::try_from(corners.len()).map_err(|_| CodeExpansionError::ResourceLimit {
            resource: "direct FilletSet corners",
            actual: corners.len(),
            limit: usize::from(u16::MAX),
        })?;
    if child_count == 0 {
        return invalid_declaration(
            declaration,
            "FilletSet must contain at least one corner".into(),
        );
    }

    let alias = semantic_alias("computed", &builder.project, &declaration.symbol, &[])?;
    let mut draft = IntentNodeDraft::new(kind, alias.clone())
        .with_dynamic_children(child_count)
        .with_field(field_key("radius")?, length(radius));
    draft.suppressed = boolean(
        required(arguments, "suppressed", &declaration.symbol.0)?,
        "FilletSet suppressed",
    )?;

    for (ordinal, value) in corners.iter().enumerate() {
        let corner = object(value, "FilletSet corner")?;
        exact_object_keys(
            corner,
            &["endpointOrder", "parents", "sweep"],
            "FilletSet corner",
        )?;
        let parents = array(
            required(corner, "parents", "FilletSet corner")?,
            "FilletSet parents",
        )?;
        let [first, second] = parents else {
            return invalid_declaration(
                declaration,
                "FilletSet corner parents must contain exactly two entries".into(),
            );
        };
        for (parent_offset, (name, value)) in ["first", "second"]
            .into_iter()
            .zip([first, second])
            .enumerate()
        {
            let parent = object(value, "FilletSet parent")?;
            exact_object_keys(
                parent,
                &[
                    "neighborhood",
                    "normalSide",
                    "parameter",
                    "periodicAnchor",
                    "retainedEndpoint",
                    "span",
                    "winding",
                ],
                "FilletSet parent",
            )?;
            let reference = format!(
                "{}.corners[{ordinal}].parents[{parent_offset}].span",
                declaration.symbol.0
            );
            let resolved = builder.resolve_managed(
                required(parent, "span", "FilletSet parent")?,
                &SemanticOutputPath::default(),
                &reference,
            )?;
            if matches!(
                &resolved,
                SemanticValue::HostOutput {
                    kind: FeatureKind::CurveSpan,
                    ..
                }
            ) {
                return Err(CodeExpansionError::Unsupported(format!(
                    "`{reference}` must reference an Intent-backed native span, not a computed host output"
                )));
            }
            let span = resolved.as_port(IntentPortKind::CurveSpan, &reference)?;
            let input_index = ordinal
                .checked_mul(2)
                .and_then(|index| index.checked_add(parent_offset))
                .and_then(|index| u16::try_from(index).ok())
                .ok_or_else(|| CodeExpansionError::ResourceLimit {
                    resource: "direct FilletSet span inputs",
                    actual: corners.len().saturating_mul(2),
                    limit: usize::from(u16::MAX),
                })?;
            draft = draft.with_input(
                InputSlot::new(InputRole::Span, input_index),
                span.patch_ref(),
            );
            draft = with_direct_fillet_parent_fields(draft, ordinal, name, parent)?;
        }
        let prefix = format!("corner_{ordinal:04}");
        let endpoint_order = match string(
            required(corner, "endpointOrder", "FilletSet corner")?,
            "FilletSet endpointOrder",
        )? {
            "firstThenSecond" => "first_then_second",
            "secondThenFirst" => "second_then_first",
            _ => {
                return invalid_declaration(
                    declaration,
                    "FilletSet endpointOrder must be firstThenSecond or secondThenFirst".into(),
                );
            }
        };
        let sweep = match string(
            required(corner, "sweep", "FilletSet corner")?,
            "FilletSet sweep",
        )? {
            "counterClockwise" => "counter_clockwise",
            "clockwise" => "clockwise",
            _ => {
                return invalid_declaration(
                    declaration,
                    "FilletSet sweep must be counterClockwise or clockwise".into(),
                );
            }
        };
        draft = draft
            .with_field(
                field_key(&format!("{prefix}_endpoint_order"))?,
                enum_value(endpoint_order)?,
            )
            .with_field(field_key(&format!("{prefix}_sweep"))?, enum_value(sweep)?);
    }

    builder.push_node(&declaration.symbol, alias.clone(), draft)?;
    builder.insert_declaration(
        declaration.symbol.clone(),
        SemanticDeclaration {
            root: SemanticValue::Declaration {
                alias,
                kind: FeatureKind::Feature,
            },
            paths: BTreeMap::new(),
        },
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "one parent-state lowering keeps its exact persisted branch fields auditable together"
)]
fn with_direct_fillet_parent_fields(
    mut draft: IntentNodeDraft,
    ordinal: usize,
    name: &str,
    parent: &BTreeMap<String, ManagedValue>,
) -> Result<IntentNodeDraft, CodeExpansionError> {
    let prefix = format!("corner_{ordinal:04}_{name}");
    let parameter = finite_number(
        required(parent, "parameter", "FilletSet parent")?,
        "FilletSet parameter",
    )?;
    let winding = integer_value(
        required(parent, "winding", "FilletSet parent")?,
        "FilletSet winding",
    )?;
    let normal_side @ ("left" | "right") = string(
        required(parent, "normalSide", "FilletSet parent")?,
        "FilletSet normalSide",
    )?
    else {
        return Err(CodeExpansionError::Unsupported(
            "FilletSet normalSide must be left or right".into(),
        ));
    };
    let retained_endpoint @ ("start" | "end") = string(
        required(parent, "retainedEndpoint", "FilletSet parent")?,
        "FilletSet retainedEndpoint",
    )?
    else {
        return Err(CodeExpansionError::Unsupported(
            "FilletSet retainedEndpoint must be start or end".into(),
        ));
    };
    draft = draft
        .with_field(
            field_key(&format!("{prefix}_parameter"))?,
            dimensionless(parameter),
        )
        .with_field(
            field_key(&format!("{prefix}_winding"))?,
            IntentLiteral::Integer(i64::from(winding)),
        )
        .with_field(
            field_key(&format!("{prefix}_normal_side"))?,
            enum_value(normal_side)?,
        )
        .with_field(
            field_key(&format!("{prefix}_trim_endpoint"))?,
            enum_value(retained_endpoint)?,
        );

    let neighborhood = object(
        required(parent, "neighborhood", "FilletSet parent")?,
        "FilletSet neighborhood",
    )?;
    let kind = string(
        required(neighborhood, "kind", "FilletSet neighborhood")?,
        "FilletSet neighborhood kind",
    )?;
    match kind {
        value @ ("interior" | "start" | "end") => {
            exact_object_keys(neighborhood, &["kind"], "FilletSet neighborhood")?;
            draft = draft.with_field(
                field_key(&format!("{prefix}_neighborhood"))?,
                enum_value(value)?,
            );
        }
        "local" => {
            exact_object_keys(
                neighborhood,
                &["kind", "lower", "upper"],
                "FilletSet neighborhood",
            )?;
            let lower = finite_number(
                required(neighborhood, "lower", "FilletSet neighborhood")?,
                "FilletSet local lower",
            )?;
            let upper = finite_number(
                required(neighborhood, "upper", "FilletSet neighborhood")?,
                "FilletSet local upper",
            )?;
            if lower >= upper {
                return Err(CodeExpansionError::Unsupported(
                    "FilletSet local neighborhood bounds must be strictly increasing".into(),
                ));
            }
            draft = draft
                .with_field(
                    field_key(&format!("{prefix}_neighborhood"))?,
                    enum_value("local")?,
                )
                .with_field(
                    field_key(&format!("{prefix}_local_lower"))?,
                    dimensionless(lower),
                )
                .with_field(
                    field_key(&format!("{prefix}_local_upper"))?,
                    dimensionless(upper),
                );
        }
        _ => {
            return Err(CodeExpansionError::Unsupported(
                "FilletSet neighborhood kind is invalid".into(),
            ));
        }
    }

    match required(parent, "periodicAnchor", "FilletSet parent")? {
        ManagedValue::Null => {
            draft = draft.with_field(
                field_key(&format!("{prefix}_periodic_anchor"))?,
                IntentLiteral::Boolean(false),
            );
        }
        value => {
            let anchor = object(value, "FilletSet periodicAnchor")?;
            exact_object_keys(
                anchor,
                &["parameter", "winding"],
                "FilletSet periodicAnchor",
            )?;
            let parameter = finite_number(
                required(anchor, "parameter", "FilletSet periodicAnchor")?,
                "FilletSet anchor parameter",
            )?;
            let winding = integer_value(
                required(anchor, "winding", "FilletSet periodicAnchor")?,
                "FilletSet anchor winding",
            )?;
            draft = draft
                .with_field(
                    field_key(&format!("{prefix}_periodic_anchor"))?,
                    IntentLiteral::Boolean(true),
                )
                .with_field(
                    field_key(&format!("{prefix}_anchor_parameter"))?,
                    dimensionless(parameter),
                )
                .with_field(
                    field_key(&format!("{prefix}_anchor_winding"))?,
                    IntentLiteral::Integer(i64::from(winding)),
                );
        }
    }
    Ok(draft)
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
    let owner = direct_owner_identity(declaration, "line", reconciliation);
    let alias = semantic_alias("decl", &builder.project, &declaration.symbol, &[])?;
    let mut draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        },
        alias.clone(),
    );
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
        let address = owner.map(|identity| {
            direct_point_address(
                &builder.project,
                &declaration.symbol,
                identity,
                fields_path(&[name]),
            )
        });
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
    reason = "one keyed Polyline lowering keeps point, span, aggregate and typed corner paths auditable together"
)]
fn lower_direct_polyline(
    builder: &mut ExpansionBuilder,
    declaration: &AuthoringDeclaration,
    reconciliation: Option<&KeyedReconcileState>,
    overlay: &CodeInteractionOverlay,
) -> Result<(), CodeExpansionError> {
    let definition = keyed_polyline_definition(declaration)?;
    let segment_count = if definition.closed {
        definition.vertices.len()
    } else {
        definition.vertices.len() - 1
    };
    let effective_vertices = definition
        .vertices
        .iter()
        .map(|(key, position)| {
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
            let legacy = overridden_point(reconciliation, &address, *position)?;
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
    let mut keyed = Vec::with_capacity(vertices.len());
    for vertex in vertices {
        let value = object(vertex, "Polyline vertex")?;
        let key = string(required(value, "key", "Polyline vertex")?, "vertex key")?;
        if keyed.iter().any(|(existing, _)| existing == key) {
            return invalid_declaration(declaration, format!("duplicate Polyline key `{key}`"));
        }
        let position = point(required(value, "position", "Polyline vertex")?, "position")?;
        keyed.push((key.to_owned(), position));
    }
    Ok(KeyedPolylineDefinition {
        vertices: keyed,
        closed,
    })
}

fn lower_direct_rectangle(
    builder: &mut ExpansionBuilder,
    declaration: &AuthoringDeclaration,
    reconciliation: Option<&KeyedReconcileState>,
    overlay: &CodeInteractionOverlay,
) -> Result<(), CodeExpansionError> {
    let arguments = object(&declaration.arguments, &declaration.symbol.0)?;
    let authored_lower_left = point(
        required(arguments, "lowerLeft", &declaration.symbol.0)?,
        "lowerLeft",
    )?;
    let authored_upper_right = point(
        required(arguments, "upperRight", &declaration.symbol.0)?,
        "upperRight",
    )?;
    let owner = direct_owner_identity(declaration, "rectangle", reconciliation);
    let (lower_address, upper_address) = owner.map_or((None, None), |identity| {
        (
            Some(direct_point_address(
                &builder.project,
                &declaration.symbol,
                identity,
                fields_path(&["lowerLeft"]),
            )),
            Some(direct_point_address(
                &builder.project,
                &declaration.symbol,
                identity,
                fields_path(&["upperRight"]),
            )),
        )
    });
    let lower_left = lower_address
        .as_ref()
        .and_then(|address| overlay_point(overlay, address))
        .unwrap_or(authored_lower_left);
    let upper_right = upper_address
        .as_ref()
        .and_then(|address| overlay_point(overlay, address))
        .unwrap_or(authored_upper_right);
    let rectangle = build_rectangle(builder, &declaration.symbol, None, lower_left, upper_right)?;
    if let (Some(lower_left_address), Some(upper_right_address)) = (lower_address, upper_address) {
        for (name, corner) in [
            ("lowerLeft", CodeRectangleCorner::LowerLeft),
            ("lowerRight", CodeRectangleCorner::LowerRight),
            ("upperRight", CodeRectangleCorner::UpperRight),
            ("upperLeft", CodeRectangleCorner::UpperLeft),
        ] {
            let point = rectangle
                .paths
                .get(&fields_path(&["corners", name]))
                .expect("built rectangle owns every corner")
                .as_corner(name)?
                .point;
            builder.add_writable_point(ExpandedWritablePoint {
                handle: point,
                source: CodePointSeedSource::Literal,
                edit: CodePointEdit::RectangleCorner {
                    lower_left: lower_left_address.clone(),
                    upper_right: upper_right_address.clone(),
                    corner,
                    effective_lower_left: lower_left,
                    effective_upper_right: upper_right,
                },
            });
        }
    }
    builder.insert_declaration(declaration.symbol.clone(), rectangle)
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
            for output in template.outputs.keys() {
                let address = GeneratedMemberAddress::new(
                    declaration.symbol.0.clone(),
                    template.path.clone(),
                    member_key.clone(),
                    [output.clone()],
                );
                let key = (template.path.clone(), member_key.clone(), output.clone());
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
                template.inputs.values().all(|binding| match binding {
                    TemplateBinding::TemplateOutput { template, .. } => artifact
                        .templates
                        .iter()
                        .find(|candidate| &candidate.path == template)
                        .is_some_and(|dependency| {
                            dependency.outputs.keys().all(|output| {
                                template_outputs.keys().any(|(path, _, candidate)| {
                                    path == template && candidate == output
                                })
                            })
                        }),
                    TemplateBinding::Input { .. } | TemplateBinding::CollectionMember { .. } => {
                        true
                    }
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
    template_outputs: &mut TemplateOutputPlan,
) -> Result<(), CodeExpansionError> {
    let member_values = collection_members_for_template(plan, &template.path);
    for (member_key, member_value) in member_values {
        let outputs = template
            .outputs
            .keys()
            .map(|output| {
                let key = (template.path.clone(), member_key.clone(), output.clone());
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
                Ok((output.clone(), address.clone(), identity))
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
        let lowered = lower_template_family(
            builder,
            plan,
            template,
            &member_key,
            &outputs,
            &bindings,
            overlay,
        )?;
        for (output, address, identity) in outputs {
            let value = lowered.get(&output).cloned().ok_or_else(|| {
                CodeExpansionError::Unsupported(format!(
                    "family `{}` did not lower output `{output}`",
                    template.declaration_family
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
    builder: &ExpansionBuilder,
    plan: &InvocationPlan,
    template: &PatchTemplateNode,
    member_key: &[String],
    member_value: Option<&SemanticValue>,
    template_outputs: &TemplateOutputPlan,
) -> Result<BTreeMap<String, SemanticValue>, CodeExpansionError> {
    let arguments = object(
        &plan
            .declaration
            .patch
            .as_ref()
            .expect("patch plan")
            .arguments,
        &plan.declaration.symbol.0,
    )?;
    template
        .inputs
        .iter()
        .map(|(name, binding)| {
            let value = match binding {
                TemplateBinding::Input {
                    name: input,
                    path,
                    expected_kind,
                } => {
                    let argument = required(arguments, input, &plan.declaration.symbol.0)?;
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
                    output,
                    expected_kind,
                } => {
                    let exact = (template.clone(), member_key.to_vec(), output.clone());
                    let fallback = (template.clone(), Vec::new(), output.clone());
                    let value = template_outputs
                        .get(&exact)
                        .or_else(|| template_outputs.get(&fallback))
                        .cloned()
                        .ok_or_else(|| CodeExpansionError::UnresolvedReference {
                            reference: format!("template {}.{output}", template.join(".")),
                        })?;
                    ensure_feature_kind(value, *expected_kind, output)?
                }
            };
            Ok((name.clone(), value))
        })
        .collect()
}

fn lower_template_family(
    builder: &mut ExpansionBuilder,
    plan: &InvocationPlan,
    template: &PatchTemplateNode,
    member_key: &[String],
    outputs: &[(String, GeneratedMemberAddress, GeneratedMemberIdentity)],
    bindings: &BTreeMap<String, SemanticValue>,
    overlay: &CodeInteractionOverlay,
) -> Result<BTreeMap<String, SemanticValue>, CodeExpansionError> {
    match code_declaration_family(&template.declaration_family)
        .and_then(|descriptor| descriptor.template)
    {
        Some(TemplateDeclarationLowering::Line) => lower_generated_line(
            builder,
            &plan.declaration.symbol,
            template,
            outputs,
            bindings,
            overlay,
        ),
        Some(TemplateDeclarationLowering::Circle) => {
            lower_generated_circle(builder, plan, outputs, bindings, overlay)
        }
        Some(TemplateDeclarationLowering::Rectangle) => {
            lower_generated_rectangle(builder, plan, outputs, overlay)
        }
        Some(TemplateDeclarationLowering::Fillet) => {
            lower_generated_fillet(builder, plan, member_key, outputs, bindings)
        }
        Some(TemplateDeclarationLowering::Profile | TemplateDeclarationLowering::Chain) => {
            lower_generated_aggregate(
                builder,
                &plan.declaration.symbol,
                template,
                outputs,
                bindings,
            )
        }
        None => Err(CodeExpansionError::Unsupported(format!(
            "artifact declaration family `{}`",
            template.declaration_family,
        ))),
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one generated-line lowering keeps bindings, detached seeds, branch state, and writable provenance together"
)]
fn lower_generated_line(
    builder: &mut ExpansionBuilder,
    declaration: &SemanticSymbol,
    template: &PatchTemplateNode,
    outputs: &[(String, GeneratedMemberAddress, GeneratedMemberIdentity)],
    bindings: &BTreeMap<String, SemanticValue>,
    overlay: &CodeInteractionOverlay,
) -> Result<BTreeMap<String, SemanticValue>, CodeExpansionError> {
    let points = bindings
        .iter()
        .filter_map(|(name, value)| {
            value
                .as_port(IntentPortKind::Point, name)
                .ok()
                .map(|port| (name.as_str(), port))
        })
        .collect::<Vec<_>>();
    let (start, end) =
        if let (Some(start), Some(end)) = (bindings.get("start"), bindings.get("end")) {
            (
                start.as_port(IntentPortKind::Point, "start")?,
                end.as_port(IntentPortKind::Point, "end")?,
            )
        } else if points.len() == 2 {
            (points[0].1.clone(), points[1].1.clone())
        } else {
            return Err(CodeExpansionError::Unsupported(format!(
                "line template `{}` must bind exactly two points",
                template.path.join(".")
            )));
        };
    let (_, address, identity) = outputs
        .first()
        .ok_or_else(|| CodeExpansionError::Unsupported("line template has no output".into()))?;
    let alias = generated_alias(address, *identity)?;
    let mut draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        },
        alias.clone(),
    );
    let mut writable = Vec::new();
    let mut endpoint_seeds = [None, None];
    for (index, (name, point)) in [("start", start), ("end", end)].into_iter().enumerate() {
        let writable_address = CodeWritableAddress::generated_point(
            builder.project.clone(),
            address.clone(),
            *identity,
            fields_path(&[name]),
        );
        let selector = node_selector(
            if index == 0 {
                IntentPortRole::Start
            } else {
                IntentPortRole::End
            },
            0,
        );
        if let Some(position) = overlay_point(overlay, &writable_address) {
            endpoint_seeds[index] = Some(position);
            draft = draft
                .with_instance_leaf(selector, LeafField::X, length(position[0]))
                .with_instance_leaf(selector, LeafField::Y, length(position[1]));
        } else {
            endpoint_seeds[index] = builder.point_seed(&SemanticValue::Port(point.clone()));
            draft = draft.with_input(
                InputSlot::new(
                    InputRole::Point,
                    u16::try_from(index).expect("two endpoints"),
                ),
                point.patch_ref(),
            );
        }
        writable.push(ExpandedWritablePoint {
            handle: port(&alias, selector, IntentPortKind::Point),
            source: CodePointSeedSource::GeneratedReference,
            edit: CodePointEdit::Point {
                address: writable_address,
            },
        });
    }
    let [Some(start_seed), Some(end_seed)] = endpoint_seeds else {
        return Err(CodeExpansionError::Unsupported(format!(
            "generated line `{}` has no deterministic endpoint seed for explicit branch direction",
            declaration.0
        )));
    };
    let direction = unit_direction(start_seed, end_seed).ok_or_else(|| {
        CodeExpansionError::InvalidDeclaration {
            declaration: declaration.0.clone(),
            message: "generated line endpoints must be finite and distinct".into(),
        }
    })?;
    draft = draft.with_field(
        field_key("branch_direction")?,
        IntentLiteral::Point(direction),
    );
    builder.push_node(declaration, alias.clone(), draft)?;
    for (role, position) in [IntentPortRole::Start, IntentPortRole::End]
        .into_iter()
        .zip([start_seed, end_seed])
    {
        builder.add_point_seed(
            &port(&alias, node_selector(role, 0), IntentPortKind::Point),
            position,
        )?;
    }
    for point in writable {
        builder.add_writable_point(point);
    }
    Ok(outputs
        .iter()
        .map(|(name, _, _)| {
            let value = match template.outputs[name] {
                FeatureKind::CurveSpan => SemanticValue::Port(port(
                    &alias,
                    node_selector(IntentPortRole::Span, 0),
                    IntentPortKind::CurveSpan,
                )),
                FeatureKind::Curve => SemanticValue::Port(port(
                    &alias,
                    node_selector(IntentPortRole::Curve, 0),
                    IntentPortKind::Curve,
                )),
                kind => SemanticValue::Declaration {
                    alias: alias.clone(),
                    kind,
                },
            };
            (name.clone(), value)
        })
        .collect())
}

#[allow(
    clippy::too_many_lines,
    reason = "one generated-circle lowering keeps typed outputs and writable provenance auditable together"
)]
fn lower_generated_circle(
    builder: &mut ExpansionBuilder,
    plan: &InvocationPlan,
    outputs: &[(String, GeneratedMemberAddress, GeneratedMemberIdentity)],
    bindings: &BTreeMap<String, SemanticValue>,
    overlay: &CodeInteractionOverlay,
) -> Result<BTreeMap<String, SemanticValue>, CodeExpansionError> {
    let center = bindings
        .get("center")
        .or_else(|| bindings.get("key"))
        .or_else(|| {
            bindings
                .values()
                .find(|value| value.kind() == FeatureKind::Point)
        })
        .ok_or_else(|| CodeExpansionError::UnresolvedReference {
            reference: format!("{}.circle.center", plan.declaration.symbol.0),
        })?;
    let radius = bindings
        .get("radius")
        .and_then(|value| match value {
            SemanticValue::ScalarLiteral(value) => Some(value.clone()),
            _ => None,
        })
        .map_or_else(|| invocation_unit(plan, &["radius", "holeRadius"]), Ok)?;
    if radius.value <= 0.0 {
        return Err(CodeExpansionError::InvalidDeclaration {
            declaration: plan.declaration.symbol.0.clone(),
            message: "circle radius must be positive".into(),
        });
    }
    let (_, address, identity) = outputs
        .first()
        .ok_or_else(|| CodeExpansionError::Unsupported("circle template has no output".into()))?;
    let alias = generated_alias(address, *identity)?;
    let writable_address = CodeWritableAddress::generated_point(
        builder.project.clone(),
        address.clone(),
        *identity,
        fields_path(&["center"]),
    );
    let drafted_center = overlay_point(overlay, &writable_address);
    let center_source = if matches!(center, SemanticValue::Port(_) | SemanticValue::Corner(_)) {
        CodePointSeedSource::GeneratedReference
    } else {
        CodePointSeedSource::Generated
    };
    let mut draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::CenterRadiusCircle,
        },
        alias.clone(),
    )
    .with_instance_leaf(
        node_selector(IntentPortRole::Target, 0),
        LeafField::Value,
        length(radius.value),
    );
    let effective_center =
        drafted_center.map_or_else(|| center.clone(), SemanticValue::PointLiteral);
    let center_seed = builder.point_seed(&effective_center).ok_or_else(|| {
        CodeExpansionError::Unsupported(format!(
            "generated circle `{}` has no deterministic center seed",
            plan.declaration.symbol.0
        ))
    })?;
    match &effective_center {
        SemanticValue::PointLiteral(position) => {
            let selector = node_selector(IntentPortRole::Center, 0);
            draft = draft
                .with_instance_leaf(selector, LeafField::X, length(position[0]))
                .with_instance_leaf(selector, LeafField::Y, length(position[1]));
        }
        value => {
            let center = value.as_port(IntentPortKind::Point, "circle center")?;
            draft = draft.with_input(InputSlot::new(InputRole::Point, 0), center.patch_ref());
        }
    }
    builder.push_node(&plan.declaration.symbol, alias.clone(), draft)?;
    builder.add_point_seed(
        &port(
            &alias,
            node_selector(IntentPortRole::Center, 0),
            IntentPortKind::Point,
        ),
        center_seed,
    )?;
    builder.add_writable_point(ExpandedWritablePoint {
        handle: port(
            &alias,
            node_selector(IntentPortRole::Center, 0),
            IntentPortKind::Point,
        ),
        source: center_source,
        edit: CodePointEdit::Point {
            address: writable_address,
        },
    });
    Ok(outputs
        .iter()
        .map(|(name, _, _)| {
            let value = match plan
                .pinned
                .artifact
                .artifact()
                .templates
                .iter()
                .find(|template| template.outputs.contains_key(name))
                .and_then(|template| template.outputs.get(name))
                .copied()
            {
                Some(FeatureKind::CurveSpan) => SemanticValue::Port(port(
                    &alias,
                    node_selector(IntentPortRole::Span, 0),
                    IntentPortKind::CurveSpan,
                )),
                _ => SemanticValue::Port(port(
                    &alias,
                    node_selector(IntentPortRole::Curve, 0),
                    IntentPortKind::Curve,
                )),
            };
            (name.clone(), value)
        })
        .collect())
}

#[allow(
    clippy::too_many_lines,
    reason = "one generated-rectangle lowering keeps coupled seeds, outputs, and provenance auditable together"
)]
fn lower_generated_rectangle(
    builder: &mut ExpansionBuilder,
    plan: &InvocationPlan,
    outputs: &[(String, GeneratedMemberAddress, GeneratedMemberIdentity)],
    overlay: &CodeInteractionOverlay,
) -> Result<BTreeMap<String, SemanticValue>, CodeExpansionError> {
    let width = invocation_number(plan, "width")?;
    let height = invocation_number(plan, "height")?;
    if width <= 0.0 || height <= 0.0 {
        return Err(CodeExpansionError::InvalidDeclaration {
            declaration: plan.declaration.symbol.0.clone(),
            message: "rectangle width and height must be positive".into(),
        });
    }
    let (_, address, identity) = outputs
        .iter()
        .find(|(name, _, _)| name == "profile")
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
        .map(|(name, address, identity)| {
            if name == "profile" {
                return Ok((name.clone(), profile.clone()));
            }
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
                name.clone(),
                SemanticValue::Port(port(&alias, selector, IntentPortKind::Point)),
            ))
        })
        .collect()
}

fn lower_generated_fillet(
    builder: &mut ExpansionBuilder,
    plan: &InvocationPlan,
    member_key: &[String],
    outputs: &[(String, GeneratedMemberAddress, GeneratedMemberIdentity)],
    bindings: &BTreeMap<String, SemanticValue>,
) -> Result<BTreeMap<String, SemanticValue>, CodeExpansionError> {
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
    for (output, address, identity) in outputs {
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
                kind: FeatureKind::CurveSpan,
            },
        );
    }
    Ok(result)
}

fn lower_generated_aggregate(
    builder: &mut ExpansionBuilder,
    declaration: &SemanticSymbol,
    template: &PatchTemplateNode,
    outputs: &[(String, GeneratedMemberAddress, GeneratedMemberIdentity)],
    bindings: &BTreeMap<String, SemanticValue>,
) -> Result<BTreeMap<String, SemanticValue>, CodeExpansionError> {
    let spans = bindings
        .iter()
        .map(|(name, value)| value.as_port(IntentPortKind::CurveSpan, name))
        .collect::<Result<Vec<_>, _>>()?;
    if spans.is_empty() {
        return Err(CodeExpansionError::Unsupported(format!(
            "aggregate `{}` has no span inputs",
            template.path.join(".")
        )));
    }
    let (_, address, identity) = outputs.first().ok_or_else(|| {
        CodeExpansionError::Unsupported("aggregate template has no output".into())
    })?;
    let alias = generated_alias(address, *identity)?;
    let aggregate = if template.declaration_family == "aggregate.profile" {
        AggregateKind::ClosedProfile
    } else {
        AggregateKind::OpenChain
    };
    let mut draft = IntentNodeDraft::new(IntentNodeKind::Aggregate { aggregate }, alias.clone());
    for (index, span) in spans.iter().enumerate() {
        let index = u16::try_from(index).map_err(|_| CodeExpansionError::ResourceLimit {
            resource: "aggregate spans",
            actual: spans.len(),
            limit: usize::from(u16::MAX),
        })?;
        draft = draft.with_input(InputSlot::new(InputRole::Span, index), span.patch_ref());
    }
    builder.push_node(declaration, alias.clone(), draft)?;
    let (role, kind) = match aggregate {
        AggregateKind::ClosedProfile => (IntentPortRole::Profile, IntentPortKind::Profile),
        AggregateKind::OpenChain => (IntentPortRole::Chain, IntentPortKind::Chain),
    };
    Ok(outputs
        .iter()
        .map(|(name, _, _)| {
            (
                name.clone(),
                SemanticValue::Port(port(&alias, node_selector(role, 0), kind)),
            )
        })
        .collect())
}

fn publish_invocation_declaration(
    builder: &mut ExpansionBuilder,
    plan: &InvocationPlan,
    outputs: &TemplateOutputPlan,
) -> Result<(), CodeExpansionError> {
    let (mut paths, mut root_members) = publish_invocation_collections(plan, outputs)?;
    for ((template, member, output), value) in outputs {
        if member.as_slice() == ["self"] {
            let mut path = template.clone();
            path.push(output.clone());
            paths.insert(
                SemanticOutputPath(path.into_iter().map(ManagedPathSegment::Field).collect()),
                value.clone(),
            );
            let template_node = plan
                .pinned
                .artifact
                .artifact()
                .templates
                .iter()
                .find(|candidate| candidate.path == *template)
                .expect("planned output belongs to one authenticated template");
            // The caller-owned compiler records the exact selected output
            // independently of its renamed or nested public path.
            if template_node.result_output.as_ref() == Some(output) {
                paths.insert(
                    SemanticOutputPath(
                        template
                            .iter()
                            .cloned()
                            .map(ManagedPathSegment::Field)
                            .collect(),
                    ),
                    value.clone(),
                );
                insert_nested_root_member(
                    &mut root_members,
                    template,
                    value.clone(),
                    &plan.declaration.symbol,
                )?;
            }
        }
    }
    let root_kind = if plan.pinned.artifact.artifact().outputs.len() == 1
        && plan.pinned.artifact.artifact().outputs.values().next() == Some(&FeatureKind::Collection)
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
                    path == template && member == &vec![key.clone()]
                }) {
                    if template_node.result_output.as_ref() == Some(output) {
                        member_outputs.insert(output.clone(), value.clone());
                    }
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

fn lower_constraints(
    project: &CodeProject,
    builder: &mut ExpansionBuilder,
) -> Result<(), CodeExpansionError> {
    for declaration in &project.managed.program.declarations {
        if declaration.patch.is_some()
            || declaration.builder_path.first().map(String::as_str) != Some("constraint")
        {
            continue;
        }
        let family = declaration.builder_path.join(".");
        match code_declaration_family(&family).and_then(|descriptor| descriptor.direct) {
            Some(DirectDeclarationLowering::Constraint(constraint)) => {
                let relation_name = match constraint {
                    ConstraintKind::Horizontal => "horizontal",
                    ConstraintKind::Vertical => "vertical",
                    _ => unreachable!("closed catalog admits only managed axis constraints"),
                };
                let arguments = object(&declaration.arguments, &declaration.symbol.0)?;
                let curve = required(arguments, "curve", &declaration.symbol.0)?;
                let value = builder.resolve_managed(
                    curve,
                    &SemanticOutputPath::default(),
                    &format!("{}.curve", declaration.symbol.0),
                )?;
                let span = match value
                    .as_port(IntentPortKind::CurveSpan, &format!("{relation_name} curve"))
                {
                    Ok(span) => span,
                    Err(error) => {
                        let ManagedValue::Reference {
                            declaration: owner,
                            path,
                        } = curve
                        else {
                            return Err(error);
                        };
                        let SemanticValue::Declaration {
                            kind: FeatureKind::Feature,
                            ..
                        } = value
                        else {
                            return Err(error);
                        };
                        builder
                            .resolve(owner, &join_paths(path, &fields_path(&["span"])))?
                            .as_port(
                                IntentPortKind::CurveSpan,
                                &format!("{relation_name} line span"),
                            )?
                    }
                };
                let alias =
                    semantic_alias("constraint", &builder.project, &declaration.symbol, &[])?;
                let mut draft =
                    IntentNodeDraft::new(IntentNodeKind::Constraint { constraint }, alias.clone())
                        .with_input(InputSlot::new(InputRole::Span, 0), span.patch_ref());
                draft.suppressed = arguments
                    .get("suppressed")
                    .map(|value| boolean(value, "suppressed"))
                    .transpose()?
                    .unwrap_or(false);
                builder.push_node(&declaration.symbol, alias.clone(), draft)?;
                builder.insert_declaration(
                    declaration.symbol.clone(),
                    SemanticDeclaration {
                        root: SemanticValue::Port(port(
                            &alias,
                            node_selector(IntentPortRole::Constraint, 0),
                            IntentPortKind::Constraint,
                        )),
                        paths: BTreeMap::new(),
                    },
                )?;
            }
            _ => {
                return Err(CodeExpansionError::Unsupported(format!(
                    "managed constraint family `{}`",
                    declaration.builder_path.join(".")
                )));
            }
        }
    }
    Ok(())
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
    let SemanticValue::Collection(members) = value else {
        return None;
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
        | IntentPortKind::Contact
        | IntentPortKind::Parameter
        | IntentPortKind::ParameterBinding
        | IntentPortKind::ParameterOutput
        | IntentPortKind::ExternalBinding
        | IntentPortKind::SemanticCatalog
        | IntentPortKind::Source
        | IntentPortKind::Annotation => FeatureKind::Scalar,
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

const fn node_selector(role: IntentPortRole, index: u16) -> IntentPortSelector {
    IntentPortSelector::Node { role, index }
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

const fn dimensionless(value: f64) -> IntentLiteral {
    IntentLiteral::Quantity {
        value,
        unit: IntentUnit::Dimensionless,
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
