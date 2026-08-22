// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ids::{
    LineageDeveloperKey, LineageDocumentId, LineageOpaqueId, LineageOutputId, LineageReservationId,
    LineageSemanticKey, LineageStepId,
};

/// Exact semantic kind of a lineage output port.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LineageOutputKind {
    Point,
    Scalar,
    Curve,
    CurveSpan,
    TrimView,
    Contact,
    Constraint,
    Dimension,
    Source,
    Parameter,
    ParameterBinding,
    ParameterOutput,
    ExternalBinding,
    GeometryRole,
    Activation,
    Profile,
    Chain,
    Operation,
    Feature,
    FeatureCorner,
    Annotation,
    Collection,
}

/// Stable typed reference to one logical output in one lineage document.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LineageOutputRef {
    pub document: LineageDocumentId,
    pub step: LineageStepId,
    pub output: LineageOutputId,
    pub kind: LineageOutputKind,
}

/// One named, kind-checked dependency of a versioned action.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LineageInputBinding {
    pub key: LineageSemanticKey,
    pub kind: LineageOutputKind,
    pub source: LineageOutputRef,
}

/// Generic versioned action body used by every admitted authoring family.
///
/// The schema name/version belongs to the action compiler. `parameters` is
/// deliberately data-only and all dependencies remain separately visible in
/// `inputs`, so structural validation never has to interpret arbitrary JSON.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VersionedActionPayload {
    pub schema: LineageSemanticKey,
    pub version: u32,
    pub inputs: Vec<LineageInputBinding>,
    pub parameters: BTreeMap<String, Value>,
}

impl VersionedActionPayload {
    /// Creates a versioned payload with no input bindings or parameters.
    #[must_use]
    pub fn empty(schema: LineageSemanticKey, version: u32) -> Self {
        Self {
            schema,
            version,
            inputs: Vec::new(),
            parameters: BTreeMap::new(),
        }
    }
}

/// Whether an imported flat baseline is supported canonical data or an opaque
/// encoding retained for truthful compatibility.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ImportedBaselineEncoding {
    Canonical {
        schema: LineageSemanticKey,
        version: u32,
    },
    Opaque {
        media_type: LineageSemanticKey,
        version: u32,
    },
}

/// Honest imported flat root. No recipe history is inferred from this payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedBaselineAction {
    pub encoding: ImportedBaselineEncoding,
    pub payload: String,
}

/// Stable action category, independent of a versioned action's family schema.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LineageActionKind {
    ImportedBaseline,
    GeometryRecipe,
    Constraint,
    Dimension,
    Trim,
    Parameter,
    Binding,
    External,
    Operation,
    ComputedFeature,
    Annotation,
}

/// Closed top-level lineage action categories with versioned family payloads.
///
/// The generic family body lets M83 carry the complete existing recipe,
/// constraint, dimension, native-operation, and computed-feature surfaces
/// without moving their equations or branch logic into this crate.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LineageActionDefinition {
    ImportedBaseline { baseline: ImportedBaselineAction },
    GeometryRecipe { action: VersionedActionPayload },
    Constraint { action: VersionedActionPayload },
    Dimension { action: VersionedActionPayload },
    Trim { action: VersionedActionPayload },
    Parameter { action: VersionedActionPayload },
    Binding { action: VersionedActionPayload },
    External { action: VersionedActionPayload },
    Operation { action: VersionedActionPayload },
    ComputedFeature { action: VersionedActionPayload },
    Annotation { action: VersionedActionPayload },
}

impl LineageActionDefinition {
    /// Returns the stable top-level action category.
    #[must_use]
    pub const fn kind(&self) -> LineageActionKind {
        match self {
            Self::ImportedBaseline { .. } => LineageActionKind::ImportedBaseline,
            Self::GeometryRecipe { .. } => LineageActionKind::GeometryRecipe,
            Self::Constraint { .. } => LineageActionKind::Constraint,
            Self::Dimension { .. } => LineageActionKind::Dimension,
            Self::Trim { .. } => LineageActionKind::Trim,
            Self::Parameter { .. } => LineageActionKind::Parameter,
            Self::Binding { .. } => LineageActionKind::Binding,
            Self::External { .. } => LineageActionKind::External,
            Self::Operation { .. } => LineageActionKind::Operation,
            Self::ComputedFeature { .. } => LineageActionKind::ComputedFeature,
            Self::Annotation { .. } => LineageActionKind::Annotation,
        }
    }

    /// Returns all structurally visible logical dependencies.
    #[must_use]
    pub fn inputs(&self) -> &[LineageInputBinding] {
        match self {
            Self::ImportedBaseline { .. } => &[],
            Self::GeometryRecipe { action }
            | Self::Constraint { action }
            | Self::Dimension { action }
            | Self::Trim { action }
            | Self::Parameter { action }
            | Self::Binding { action }
            | Self::External { action }
            | Self::Operation { action }
            | Self::ComputedFeature { action }
            | Self::Annotation { action } => &action.inputs,
        }
    }

    pub(crate) fn inputs_mut(&mut self) -> Option<&mut Vec<LineageInputBinding>> {
        match self {
            Self::ImportedBaseline { .. } => None,
            Self::GeometryRecipe { action }
            | Self::Constraint { action }
            | Self::Dimension { action }
            | Self::Trim { action }
            | Self::Parameter { action }
            | Self::Binding { action }
            | Self::External { action }
            | Self::Operation { action }
            | Self::ComputedFeature { action }
            | Self::Annotation { action } => Some(&mut action.inputs),
        }
    }

    pub(crate) fn payload(&self) -> Option<&VersionedActionPayload> {
        match self {
            Self::ImportedBaseline { .. } => None,
            Self::GeometryRecipe { action }
            | Self::Constraint { action }
            | Self::Dimension { action }
            | Self::Trim { action }
            | Self::Parameter { action }
            | Self::Binding { action }
            | Self::External { action }
            | Self::Operation { action }
            | Self::ComputedFeature { action }
            | Self::Annotation { action } => Some(action),
        }
    }

    pub(crate) fn normalize(&mut self) {
        if let Some(inputs) = self.inputs_mut() {
            inputs.sort_by(|left, right| left.key.cmp(&right.key));
        }
    }
}

/// Native/materialized identity kind reserved by a lineage step.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LineageReservationKind {
    Point,
    Scalar,
    Curve,
    CurveSpan,
    TrimView,
    Contact,
    Constraint,
    Dimension,
    Source,
    Parameter,
    ParameterBinding,
    ParameterOutput,
    ExternalBinding,
    Profile,
    Chain,
    Operation,
    Feature,
    FeatureCorner,
    Annotation,
    HiddenSupport,
}

/// One stable reservation in a step's complete materialization ownership set.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LineageReservation {
    pub id: LineageReservationId,
    pub key: LineageSemanticKey,
    pub kind: LineageReservationKind,
    pub persistent_id: LineageOpaqueId,
}

/// One stable semantic output port, optionally backed by a native reservation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LineageOutput {
    pub id: LineageOutputId,
    pub key: LineageSemanticKey,
    pub kind: LineageOutputKind,
    pub reservation: Option<LineageReservationId>,
}

/// Exact ownership and native-identity transition represented by one output.
///
/// Created outputs consume a fresh reservation. Aliased outputs reuse an
/// earlier logical port without taking ownership. Continued outputs transfer
/// an exact persistent identity through a topology operation, while retired
/// outputs make an exact predecessor permanently unavailable. `OwnedLogical`
/// is reserved for semantic outputs without a native identity, such as a
/// stable collection or profile port.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LineageOutputIdentityFlow {
    OwnedLogical,
    Created { reservation: LineageReservationId },
    Aliased { source: LineageOutputRef },
    Continued { source: LineageOutputRef },
    Retired { source: LineageOutputRef },
}

/// Identity-flow declaration for one stable output port.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LineageOutputIdentity {
    pub output: LineageOutputId,
    pub flow: LineageOutputIdentityFlow,
}

impl LineageOutputIdentity {
    /// Returns the earlier logical port consumed by an alias, continuation, or
    /// retirement transition.
    #[must_use]
    pub const fn source(&self) -> Option<LineageOutputRef> {
        match self.flow {
            LineageOutputIdentityFlow::Aliased { source }
            | LineageOutputIdentityFlow::Continued { source }
            | LineageOutputIdentityFlow::Retired { source } => Some(source),
            LineageOutputIdentityFlow::OwnedLogical | LineageOutputIdentityFlow::Created { .. } => {
                None
            }
        }
    }
}

/// One stable writable semantic leaf declared by an owning output port.
///
/// `key` identifies a value within the output's exact logical/materialized
/// identity (for example `position.x`, `position.y`, or `definition.sweep`).
/// It is never a JSON array index, display label, coordinate, or proximity
/// token. Identity flow and the output's reservation resolve the declaration
/// to current reverse write-back authority.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LineageWritableLeaf {
    pub output: LineageOutputId,
    pub key: LineageSemanticKey,
}

/// Persistent lifecycle of one retained lineage action.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LineageStepState {
    Live,
    Suppressed,
    Tombstoned,
}

/// One ordered lineage action with stable logical/materialized ownership.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LineageStep {
    pub id: LineageStepId,
    pub key: LineageDeveloperKey,
    pub label: String,
    pub state: LineageStepState,
    pub action: LineageActionDefinition,
    pub outputs: Vec<LineageOutput>,
    pub output_identities: Vec<LineageOutputIdentity>,
    #[serde(default)]
    pub writable_leaves: Vec<LineageWritableLeaf>,
    pub reservations: Vec<LineageReservation>,
}

impl LineageStep {
    /// Creates a live lineage step. Document insertion performs complete
    /// allocator, dependency, output, and reservation validation atomically.
    #[must_use]
    pub fn new(
        id: LineageStepId,
        key: LineageDeveloperKey,
        label: impl Into<String>,
        action: LineageActionDefinition,
        outputs: Vec<LineageOutput>,
        reservations: Vec<LineageReservation>,
    ) -> Self {
        let output_identities = outputs
            .iter()
            .map(|output| LineageOutputIdentity {
                output: output.id,
                flow: output
                    .reservation
                    .map_or(LineageOutputIdentityFlow::OwnedLogical, |reservation| {
                        LineageOutputIdentityFlow::Created { reservation }
                    }),
            })
            .collect();
        Self {
            id,
            key,
            label: label.into(),
            state: LineageStepState::Live,
            action,
            outputs,
            output_identities,
            writable_leaves: Vec::new(),
            reservations,
        }
    }

    /// Replaces the constructor-inferred created/logical identity manifest.
    /// Complete document validation requires exactly one declaration per
    /// output and checks every predecessor and reservation atomically.
    #[must_use]
    pub fn with_output_identities(mut self, output_identities: Vec<LineageOutputIdentity>) -> Self {
        self.output_identities = output_identities;
        self
    }

    /// Replaces the constructor's empty writable-leaf manifest.
    ///
    /// Complete document validation requires every declaration to name an
    /// output owned by this step. Ordinary rewrites deliberately preserve this
    /// manifest unchanged.
    #[must_use]
    pub fn with_writable_leaves(mut self, writable_leaves: Vec<LineageWritableLeaf>) -> Self {
        self.writable_leaves = writable_leaves;
        self
    }

    /// Returns the identity declaration for one output port.
    #[must_use]
    pub fn output_identity(&self, output: LineageOutputId) -> Option<&LineageOutputIdentity> {
        self.output_identities
            .iter()
            .find(|candidate| candidate.output == output)
    }

    /// Returns a typed reference to one output owned by this step.
    #[must_use]
    pub fn output_ref(
        &self,
        document: LineageDocumentId,
        output: LineageOutputId,
    ) -> Option<LineageOutputRef> {
        self.outputs
            .iter()
            .find(|candidate| candidate.id == output)
            .map(|candidate| LineageOutputRef {
                document,
                step: self.id,
                output: candidate.id,
                kind: candidate.kind,
            })
    }
}

pub(crate) const fn reservation_matches_output(
    reservation: LineageReservationKind,
    output: LineageOutputKind,
) -> bool {
    matches!(
        (reservation, output),
        (LineageReservationKind::Point, LineageOutputKind::Point)
            | (LineageReservationKind::Scalar, LineageOutputKind::Scalar)
            | (LineageReservationKind::Curve, LineageOutputKind::Curve)
            | (
                LineageReservationKind::CurveSpan,
                LineageOutputKind::CurveSpan
            )
            | (
                LineageReservationKind::TrimView,
                LineageOutputKind::TrimView
            )
            | (LineageReservationKind::Contact, LineageOutputKind::Contact)
            | (
                LineageReservationKind::Constraint,
                LineageOutputKind::Constraint
            )
            | (
                LineageReservationKind::Dimension,
                LineageOutputKind::Dimension
            )
            | (LineageReservationKind::Source, LineageOutputKind::Source)
            | (
                LineageReservationKind::Parameter,
                LineageOutputKind::Parameter
            )
            | (
                LineageReservationKind::ParameterBinding,
                LineageOutputKind::ParameterBinding
            )
            | (
                LineageReservationKind::ParameterOutput,
                LineageOutputKind::ParameterOutput
            )
            | (
                LineageReservationKind::ExternalBinding,
                LineageOutputKind::ExternalBinding
            )
            | (LineageReservationKind::Profile, LineageOutputKind::Profile)
            | (LineageReservationKind::Chain, LineageOutputKind::Chain)
            | (
                LineageReservationKind::Operation,
                LineageOutputKind::Operation
            )
            | (LineageReservationKind::Feature, LineageOutputKind::Feature)
            | (
                LineageReservationKind::FeatureCorner,
                LineageOutputKind::FeatureCorner
            )
            | (
                LineageReservationKind::Annotation,
                LineageOutputKind::Annotation
            )
    )
}
