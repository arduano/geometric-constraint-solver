// SPDX-License-Identifier: GPL-3.0-or-later

//! Persistent, equation-free GUI placement layered over managed code seeds.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    GeneratedMemberAddress, GeneratedMemberIdentity, ManagedPathSegment, ProjectKey,
    SemanticOutputPath, SemanticSymbol,
};

const MAX_DRAFTS: usize = 65_536;
const MAX_PATH_SEGMENTS: usize = 64;
const MAX_KEY_BYTES: usize = 256;

/// Semantic owner of a code-authored writable value.
///
/// Intent/native IDs and implementation aliases are deliberately absent.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(tag = "owner", rename_all = "snake_case", deny_unknown_fields)]
pub enum CodeOwnerAddress {
    /// A declaration written directly in the managed program.
    DirectDeclaration { declaration: SemanticSymbol },
    /// One exact generation produced by a keyed custom/direct expansion.
    GeneratedMember { address: GeneratedMemberAddress },
}

/// Generation-authenticated identity of one semantic owner.
///
/// Both direct and generated owners carry a never-reused keyed allocation and
/// generation. The semantic address remains readable and independent of that
/// allocator coordinate.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeOwnerIdentity {
    pub address: CodeOwnerAddress,
    pub allocation: u64,
    pub generation: u32,
}

impl CodeOwnerIdentity {
    #[must_use]
    pub fn direct(declaration: SemanticSymbol, identity: GeneratedMemberIdentity) -> Self {
        Self {
            address: CodeOwnerAddress::DirectDeclaration { declaration },
            allocation: identity.allocation,
            generation: identity.generation,
        }
    }

    #[must_use]
    pub fn generated(address: GeneratedMemberAddress, identity: GeneratedMemberIdentity) -> Self {
        Self {
            address: CodeOwnerAddress::GeneratedMember { address },
            allocation: identity.allocation,
            generation: identity.generation,
        }
    }

    #[must_use]
    pub const fn generated_identity(&self) -> Option<GeneratedMemberIdentity> {
        match self.address {
            CodeOwnerAddress::DirectDeclaration { .. }
            | CodeOwnerAddress::GeneratedMember { .. } => Some(GeneratedMemberIdentity {
                allocation: self.allocation,
                generation: self.generation,
            }),
        }
    }
}

/// Typed leaf family owned at one semantic output path.
///
/// The interaction overlay deliberately exposes only Cartesian point seeds. Scalar edits remain
/// ordinary managed-source lens edits until expansion can publish an equally
/// explicit writable-scalar manifest and lowering contract.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeWritableField {
    Point,
}

/// Stable code-facing coordinate of one writable Cartesian point seed.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeWritableAddress {
    pub project: ProjectKey,
    pub owner: CodeOwnerIdentity,
    pub output: SemanticOutputPath,
    pub field: CodeWritableField,
}

/// Generation-authenticated semantic address of one generated host child.
///
/// The optional child path distinguishes several native children materialized
/// from one generated output (for example the four Fillets owned by a rounded
/// rectangle profile). An empty path denotes the generated output itself.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeGeneratedChildAddress {
    pub project: ProjectKey,
    pub owner: CodeOwnerIdentity,
    pub child: SemanticOutputPath,
}

impl CodeGeneratedChildAddress {
    #[must_use]
    pub fn new(
        project: ProjectKey,
        address: GeneratedMemberAddress,
        identity: GeneratedMemberIdentity,
        child: SemanticOutputPath,
    ) -> Self {
        Self {
            project,
            owner: CodeOwnerIdentity::generated(address, identity),
            child,
        }
    }

    #[must_use]
    pub fn display_path(&self) -> String {
        let CodeOwnerAddress::GeneratedMember { address } = &self.owner.address else {
            return format!("{}::<invalid-direct-child>", self.project.0);
        };
        let child = display_output_path(&self.child);
        if child.is_empty() {
            format!("{}::{}", self.project.0, address.display_path())
        } else {
            format!("{}::{}.{child}", self.project.0, address.display_path())
        }
    }
}

impl CodeWritableAddress {
    #[must_use]
    pub fn direct_point(
        project: ProjectKey,
        declaration: SemanticSymbol,
        identity: GeneratedMemberIdentity,
        output: SemanticOutputPath,
    ) -> Self {
        Self {
            project,
            owner: CodeOwnerIdentity::direct(declaration, identity),
            output,
            field: CodeWritableField::Point,
        }
    }

    #[must_use]
    pub fn generated_point(
        project: ProjectKey,
        address: GeneratedMemberAddress,
        identity: GeneratedMemberIdentity,
        output: SemanticOutputPath,
    ) -> Self {
        Self {
            project,
            owner: CodeOwnerIdentity::generated(address, identity),
            output,
            field: CodeWritableField::Point,
        }
    }

    #[must_use]
    pub fn display_path(&self) -> String {
        let owner = match &self.owner.address {
            CodeOwnerAddress::DirectDeclaration { declaration } => declaration.0.clone(),
            CodeOwnerAddress::GeneratedMember { address } => address.display_path(),
        };
        let output = display_output_path(&self.output);
        if output.is_empty() {
            format!("{}::{owner}", self.project.0)
        } else {
            format!("{}::{owner}.{output}", self.project.0)
        }
    }
}

/// Why a durable GUI draft exists. This is audit information, not solve
/// priority; all variants remain ordinary instance seeds.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeDraftProvenance {
    CanvasDrag,
    DetachedReference,
    GeneratedOverride,
}

/// Finite equation-free placement supplied by the GUI.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(tag = "value", content = "seed", rename_all = "snake_case")]
pub enum CodeDraftValue {
    Point([f64; 2]),
}

impl PartialEq for CodeDraftValue {
    fn eq(&self, other: &Self) -> bool {
        match (*self, *other) {
            (Self::Point(left), Self::Point(right)) => point_bits(left) == point_bits(right),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeDraft {
    pub value: CodeDraftValue,
    pub provenance: CodeDraftProvenance,
}

/// Bounded persistent UX placement authority. The map key is semantic and
/// generation-authenticated; native/intent identities never enter this wire.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CodeInteractionOverlay {
    drafts: BTreeMap<CodeWritableAddress, CodeDraft>,
    suppressed_children: BTreeMap<CodeGeneratedChildAddress, bool>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CodeInteractionOverlayWire {
    drafts: Vec<(CodeWritableAddress, CodeDraft)>,
    #[serde(default)]
    suppressed_children: Vec<(CodeGeneratedChildAddress, bool)>,
}

impl Serialize for CodeInteractionOverlay {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        CodeInteractionOverlayWire {
            drafts: self
                .drafts
                .iter()
                .map(|(address, draft)| (address.clone(), draft.clone()))
                .collect(),
            suppressed_children: self
                .suppressed_children
                .iter()
                .map(|(address, suppressed)| (address.clone(), *suppressed))
                .collect(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CodeInteractionOverlay {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = CodeInteractionOverlayWire::deserialize(deserializer)?;
        let count = wire.drafts.len();
        let suppressed_count = wire.suppressed_children.len();
        let overlay = Self {
            drafts: wire.drafts.into_iter().collect(),
            suppressed_children: wire.suppressed_children.into_iter().collect(),
        };
        if overlay.drafts.len() != count {
            return Err(serde::de::Error::custom("duplicate code draft address"));
        }
        if overlay.suppressed_children.len() != suppressed_count {
            return Err(serde::de::Error::custom(
                "duplicate generated-child suppression address",
            ));
        }
        overlay.validate().map_err(serde::de::Error::custom)?;
        Ok(overlay)
    }
}

impl CodeInteractionOverlay {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            drafts: BTreeMap::new(),
            suppressed_children: BTreeMap::new(),
        }
    }

    #[must_use]
    pub const fn drafts(&self) -> &BTreeMap<CodeWritableAddress, CodeDraft> {
        &self.drafts
    }

    #[must_use]
    pub fn draft(&self, address: &CodeWritableAddress) -> Option<&CodeDraft> {
        self.drafts.get(address)
    }

    #[must_use]
    pub const fn suppressed_children(&self) -> &BTreeMap<CodeGeneratedChildAddress, bool> {
        &self.suppressed_children
    }

    #[must_use]
    pub fn generated_child_suppression(&self, address: &CodeGeneratedChildAddress) -> Option<bool> {
        self.suppressed_children.get(address).copied()
    }

    /// Adds or replaces one finite point draft.
    ///
    /// # Errors
    ///
    /// Returns an address, type, finite-value, or resource-limit error before
    /// changing the overlay.
    pub fn set_point(
        &mut self,
        address: CodeWritableAddress,
        point: [f64; 2],
        provenance: CodeDraftProvenance,
    ) -> Result<(), CodeOverlayError> {
        self.set(address, CodeDraftValue::Point(point), provenance)
    }

    /// Applies one terminal drag bundle atomically. Equal duplicate writes
    /// collapse; conflicting same-tier writes reject independent of order.
    ///
    /// # Errors
    ///
    /// Returns an address, type, finite-value, resource-limit, or exact
    /// same-tier conflict error before changing the overlay.
    pub fn set_points_atomically(
        &mut self,
        updates: impl IntoIterator<Item = (CodeWritableAddress, [f64; 2])>,
        provenance: CodeDraftProvenance,
    ) -> Result<(), CodeOverlayError> {
        self.set_point_drafts_atomically(
            updates
                .into_iter()
                .map(|(address, point)| (address, point, provenance)),
        )
    }

    /// Applies one terminal bundle with per-write provenance atomically.
    /// Equal duplicate values collapse under deterministic provenance
    /// precedence; unequal writes to one semantic address reject before this
    /// overlay is changed.
    ///
    /// # Errors
    ///
    /// Returns an address, type, finite-value, resource-limit, or exact
    /// same-tier conflict error before changing the overlay.
    #[allow(
        clippy::float_cmp,
        reason = "semantic seed conflicts compare exact persisted IEEE values"
    )]
    pub fn set_point_drafts_atomically(
        &mut self,
        updates: impl IntoIterator<Item = (CodeWritableAddress, [f64; 2], CodeDraftProvenance)>,
    ) -> Result<(), CodeOverlayError> {
        let mut unique = BTreeMap::<CodeWritableAddress, ([f64; 2], CodeDraftProvenance)>::new();
        for (address, point, provenance) in updates {
            validate_address(&address)?;
            validate_value(address.field, CodeDraftValue::Point(point))?;
            if let Some((previous, previous_provenance)) = unique.get_mut(&address) {
                if point_bits(*previous) != point_bits(point) {
                    return Err(CodeOverlayError::ConflictingDraft(address.display_path()));
                }
                *previous_provenance = canonical_provenance(*previous_provenance, provenance);
            } else {
                unique.insert(address, (point, provenance));
            }
        }
        self.ensure_capacity(
            unique
                .keys()
                .filter(|address| !self.drafts.contains_key(*address))
                .count(),
        )?;
        for (address, (point, provenance)) in unique {
            self.drafts.insert(
                address,
                CodeDraft {
                    value: CodeDraftValue::Point(point),
                    provenance,
                },
            );
        }
        Ok(())
    }

    /// Sets generated-child suppression as one bounded semantic overlay
    /// action. This does not delete the owning managed invocation.
    ///
    /// # Errors
    ///
    /// Returns an address or resource-limit error before changing the overlay.
    pub fn set_generated_child_suppressed(
        &mut self,
        address: CodeGeneratedChildAddress,
        suppressed: bool,
    ) -> Result<(), CodeOverlayError> {
        validate_generated_child_address(&address)?;
        self.ensure_capacity(usize::from(
            !self.suppressed_children.contains_key(&address),
        ))?;
        self.suppressed_children.insert(address, suppressed);
        Ok(())
    }

    /// Applies a suppression bundle atomically. Equal duplicate writes
    /// collapse and contradictory writes reject independent of input order.
    ///
    /// # Errors
    ///
    /// Returns an address, resource-limit, or same-tier conflict error before
    /// changing the overlay.
    pub fn set_generated_children_suppressed_atomically(
        &mut self,
        updates: impl IntoIterator<Item = (CodeGeneratedChildAddress, bool)>,
    ) -> Result<(), CodeOverlayError> {
        let mut unique = BTreeMap::new();
        for (address, suppressed) in updates {
            validate_generated_child_address(&address)?;
            if let Some(previous) = unique.insert(address.clone(), suppressed)
                && previous != suppressed
            {
                return Err(CodeOverlayError::ConflictingSuppression(
                    address.display_path(),
                ));
            }
        }
        self.ensure_capacity(
            unique
                .keys()
                .filter(|address| !self.suppressed_children.contains_key(*address))
                .count(),
        )?;
        self.suppressed_children.extend(unique);
        Ok(())
    }

    fn set(
        &mut self,
        address: CodeWritableAddress,
        value: CodeDraftValue,
        provenance: CodeDraftProvenance,
    ) -> Result<(), CodeOverlayError> {
        validate_address(&address)?;
        validate_value(address.field, value)?;
        self.ensure_capacity(usize::from(!self.drafts.contains_key(&address)))?;
        self.drafts.insert(address, CodeDraft { value, provenance });
        Ok(())
    }

    /// Removes a draft so expansion uses code/retained placement again.
    pub fn reset(&mut self, address: &CodeWritableAddress) -> bool {
        self.drafts.remove(address).is_some()
    }

    /// Removes one complete semantic edit bundle. This is used by coupled
    /// codecs such as rectangles, where one visible corner owns both
    /// canonical source point seeds and a half-reset would be misleading.
    pub fn reset_many(
        &mut self,
        addresses: impl IntoIterator<Item = CodeWritableAddress>,
    ) -> usize {
        addresses
            .into_iter()
            .filter(|address| self.drafts.remove(address).is_some())
            .count()
    }

    pub fn reset_generated_child_suppression(
        &mut self,
        address: &CodeGeneratedChildAddress,
    ) -> bool {
        self.suppressed_children.remove(address).is_some()
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&CodeWritableAddress, &CodeDraft) -> bool) {
        self.drafts.retain(|address, draft| keep(address, draft));
    }

    pub fn retain_generated_children(
        &mut self,
        mut keep: impl FnMut(&CodeGeneratedChildAddress, bool) -> bool,
    ) {
        self.suppressed_children
            .retain(|address, suppressed| keep(address, *suppressed));
    }

    /// Revalidates all bounds, semantic addresses, types and finite values.
    ///
    /// # Errors
    ///
    /// Returns the first deterministic invalid-address, type, finite-value,
    /// or resource-limit diagnostic.
    pub fn validate(&self) -> Result<(), CodeOverlayError> {
        let total = self.entry_count();
        if total > MAX_DRAFTS {
            return Err(CodeOverlayError::ResourceLimit {
                actual: total,
                limit: MAX_DRAFTS,
            });
        }
        for (address, draft) in &self.drafts {
            validate_address(address)?;
            validate_value(address.field, draft.value)?;
        }
        for address in self.suppressed_children.keys() {
            validate_generated_child_address(address)?;
        }
        Ok(())
    }

    fn entry_count(&self) -> usize {
        self.drafts
            .len()
            .saturating_add(self.suppressed_children.len())
    }

    fn ensure_capacity(&self, added: usize) -> Result<(), CodeOverlayError> {
        let actual = self.entry_count().saturating_add(added);
        if actual > MAX_DRAFTS {
            Err(CodeOverlayError::ResourceLimit {
                actual,
                limit: MAX_DRAFTS,
            })
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum CodeOverlayError {
    #[error("code interaction overlay contains {actual} drafts; the limit is {limit}")]
    ResourceLimit { actual: usize, limit: usize },
    #[error("invalid code writable address `{0}`")]
    InvalidAddress(String),
    #[error("draft type does not match writable field at `{0}`")]
    TypeMismatch(String),
    #[error("draft at `{0}` contains a non-finite value")]
    NonFinite(String),
    #[error("conflicting same-tier drafts target `{0}`")]
    ConflictingDraft(String),
    #[error("conflicting same-tier suppression drafts target `{0}`")]
    ConflictingSuppression(String),
}

fn canonical_provenance(
    left: CodeDraftProvenance,
    right: CodeDraftProvenance,
) -> CodeDraftProvenance {
    // This is audit provenance rather than solver priority. The ordering only
    // chooses a canonical label when identical terminal values collapse.
    left.max(right)
}

fn point_bits(point: [f64; 2]) -> [u64; 2] {
    point.map(f64::to_bits)
}

fn validate_address(address: &CodeWritableAddress) -> Result<(), CodeOverlayError> {
    let invalid_project = address.project.0.is_empty()
        || address.project.0.len() > MAX_KEY_BYTES
        || address.project.0.chars().any(char::is_control);
    let owner_valid = match &address.owner.address {
        CodeOwnerAddress::DirectDeclaration { declaration } => {
            address.owner.allocation > 0 && valid_key(&declaration.0)
        }
        CodeOwnerAddress::GeneratedMember { address: generated } => {
            let segments = 1_usize
                .saturating_add(generated.template.len())
                .saturating_add(generated.member_key.len())
                .saturating_add(generated.output.len());
            address.owner.allocation > 0
                && valid_key(&generated.invocation)
                && !generated.template.is_empty()
                && !generated.member_key.is_empty()
                && !generated.output.is_empty()
                && segments <= MAX_PATH_SEGMENTS
                && std::iter::once(&generated.invocation)
                    .chain(&generated.template)
                    .chain(&generated.member_key)
                    .chain(&generated.output)
                    .all(|value| valid_key(value))
        }
    };
    let path_valid = address.output.0.len() <= MAX_PATH_SEGMENTS
        && address.output.0.iter().all(|segment| match segment {
            ManagedPathSegment::Field(value) | ManagedPathSegment::Member { member: value } => {
                valid_key(value)
            }
            ManagedPathSegment::Index(_) => true,
        });
    if invalid_project || !owner_valid || !path_valid {
        Err(CodeOverlayError::InvalidAddress(address.display_path()))
    } else {
        Ok(())
    }
}

fn validate_generated_child_address(
    address: &CodeGeneratedChildAddress,
) -> Result<(), CodeOverlayError> {
    let CodeOwnerAddress::GeneratedMember { .. } = &address.owner.address else {
        return Err(CodeOverlayError::InvalidAddress(address.display_path()));
    };
    let writable = CodeWritableAddress {
        project: address.project.clone(),
        owner: address.owner.clone(),
        output: address.child.clone(),
        field: CodeWritableField::Point,
    };
    validate_address(&writable)
}

fn display_output_path(path: &SemanticOutputPath) -> String {
    path.0
        .iter()
        .map(|segment| match segment {
            ManagedPathSegment::Field(field) => field.clone(),
            ManagedPathSegment::Index(index) => format!("[{index}]"),
            ManagedPathSegment::Member { member } => format!("[{member}]"),
        })
        .collect::<Vec<_>>()
        .join(".")
}

fn valid_key(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_KEY_BYTES && !value.chars().any(char::is_control)
}

fn validate_value(field: CodeWritableField, value: CodeDraftValue) -> Result<(), CodeOverlayError> {
    match (field, value) {
        (CodeWritableField::Point, CodeDraftValue::Point(point)) => {
            if point.into_iter().all(f64::is_finite) {
                Ok(())
            } else {
                Err(CodeOverlayError::NonFinite("point".into()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn address() -> CodeWritableAddress {
        CodeWritableAddress::direct_point(
            ProjectKey("project".into()),
            SemanticSymbol("line".into()),
            GeneratedMemberIdentity {
                allocation: 1,
                generation: 0,
            },
            SemanticOutputPath(vec![ManagedPathSegment::Field("start".into())]),
        )
    }

    #[test]
    fn overlay_is_bounded_finite_and_round_trips_canonically() {
        let mut overlay = CodeInteractionOverlay::empty();
        overlay
            .set_point(address(), [3.0, 4.0], CodeDraftProvenance::CanvasDrag)
            .unwrap();
        let json = serde_json::to_string(&overlay).unwrap();
        assert_eq!(
            serde_json::from_str::<CodeInteractionOverlay>(&json).unwrap(),
            overlay
        );
        assert!(matches!(
            overlay.set_point(address(), [f64::NAN, 0.0], CodeDraftProvenance::CanvasDrag),
            Err(CodeOverlayError::NonFinite(_))
        ));
    }

    #[test]
    fn duplicate_wire_addresses_reject() {
        let draft = CodeDraft {
            value: CodeDraftValue::Point([1.0, 2.0]),
            provenance: CodeDraftProvenance::CanvasDrag,
        };
        let json = serde_json::to_string(&serde_json::json!({
            "drafts": [[address(), draft.clone()], [address(), draft]],
        }))
        .unwrap();
        assert!(serde_json::from_str::<CodeInteractionOverlay>(&json).is_err());
    }

    #[test]
    fn signed_zero_duplicate_writes_reject_in_both_orders() {
        for points in [[[0.0, 1.0], [-0.0, 1.0]], [[-0.0, 1.0], [0.0, 1.0]]] {
            let mut overlay = CodeInteractionOverlay::empty();
            assert!(matches!(
                overlay.set_points_atomically(
                    points.into_iter().map(|point| (address(), point)),
                    CodeDraftProvenance::CanvasDrag,
                ),
                Err(CodeOverlayError::ConflictingDraft(_))
            ));
            assert!(overlay.drafts().is_empty());
        }
        assert_ne!(
            CodeDraftValue::Point([0.0, 1.0]),
            CodeDraftValue::Point([-0.0, 1.0]),
        );
    }

    #[test]
    fn display_paths_are_labels_not_semantic_identity_tokens() {
        let direct = |output| {
            CodeWritableAddress::direct_point(
                ProjectKey("project".into()),
                SemanticSymbol("line".into()),
                GeneratedMemberIdentity {
                    allocation: 1,
                    generation: 0,
                },
                SemanticOutputPath(output),
            )
        };
        let field_with_dot = direct(vec![ManagedPathSegment::Field("a.b".into())]);
        let two_fields = direct(vec![
            ManagedPathSegment::Field("a".into()),
            ManagedPathSegment::Field("b".into()),
        ]);
        assert_ne!(field_with_dot, two_fields);
        assert_eq!(field_with_dot.display_path(), two_fields.display_path());
        assert_ne!(
            serde_json::to_string(&field_with_dot).unwrap(),
            serde_json::to_string(&two_fields).unwrap(),
        );
    }

    #[test]
    fn generated_addresses_are_bounded_and_scalar_drafts_are_not_admitted() {
        let generated = |address: GeneratedMemberAddress| {
            CodeWritableAddress::generated_point(
                ProjectKey("project".into()),
                address,
                GeneratedMemberIdentity {
                    allocation: 2,
                    generation: 0,
                },
                SemanticOutputPath::default(),
            )
        };
        for invalid in [
            GeneratedMemberAddress::new("", ["template"], ["member"], ["output"]),
            GeneratedMemberAddress::new("owner", Vec::<String>::new(), ["member"], ["output"]),
            GeneratedMemberAddress::new(
                "owner",
                (0..64).map(|index| format!("template-{index}")),
                ["member".to_owned()],
                ["output".to_owned()],
            ),
        ] {
            assert!(matches!(
                CodeInteractionOverlay::empty().set_point(
                    generated(invalid),
                    [0.0, 0.0],
                    CodeDraftProvenance::GeneratedOverride,
                ),
                Err(CodeOverlayError::InvalidAddress(_))
            ));
        }

        let scalar_wire = serde_json::json!({
            "drafts": [[
                {
                    "project": "project",
                    "owner": {
                        "address": { "owner": "direct_declaration", "declaration": "line" },
                        "allocation": 1,
                        "generation": 0
                    },
                    "output": ["start"],
                    "field": "scalar"
                },
                {
                    "value": "scalar",
                    "seed": 1.0,
                    "provenance": "canvas_drag"
                }
            ]]
        });
        assert!(serde_json::from_value::<CodeInteractionOverlay>(scalar_wire).is_err());
    }
}
