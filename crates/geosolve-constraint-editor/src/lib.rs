// SPDX-License-Identifier: GPL-3.0-or-later

//! Presentation-independent interaction state for 2D constraint editors.
//!
//! This crate consumes accepted public [`geosolve_sketch`] documents, produces
//! deterministic screen-space scene primitives, resolves pointer hits to persistent
//! sketch identities, and emits typed effects for a host to apply. It owns no solver
//! equations, renderer, DOM integration, persistence, or platform event loop.

mod annotations;
mod authoring;
mod commit_plan;
mod coordinator;
mod curve_controls;
mod feature_authoring;
mod geometry_tools;
mod inference;

pub use annotations::{
    AnnotationLayoutEntry, AnnotationLayoutKey, AnnotationLayoutState, AnnotationPlacement,
    SceneAnnotation, SceneAnnotationArrowhead, SceneAnnotationGeometry, SceneAnnotationGlyphBounds,
    SceneAnnotationKind, SceneAnnotationLabelBounds, SceneAnnotationVisibility,
    SceneConstraintEntry, SceneConstraintGlyph, SceneGlyphMarker, compact_dimension_text,
    constraint_entries,
};
pub use authoring::{
    AuthoringApplication, AuthoringOperand, AuthoringOperandKind, AuthoringOptions,
    AuthoringOutcome, AuthoringState, AuthoringTool, AuthoringWarning,
};
pub use commit_plan::{
    ConstructionCommitPlan, ConstructionCommitResult, ConstructionConstraintResult,
    ConstructionContactResult, DraftContactDescriptor, DraftCurveSlot, DraftLineSupportSlot,
    DraftPointSlot, DraftSpanSlot, InferredRelation, MAX_CONSTRUCTION_PLAN_RELATIONS,
};
pub use coordinator::{
    ActionAvailability, ActionState, AuditDto, AuditProvenance, AuthoringMutation, BranchAction,
    ComputedFeatureMutation, ComputedFeatureProblemMetadata, ComputedProfileBoundary,
    ComputedSceneState, ContactBranchAction, CoordinatorActionKind, CoordinatorError,
    CurveNumericPropertyKind, CurveNumericPropertyMetadata, CurvePropertyFamily,
    DimensionTargetDisplayUnit, DimensionTargetMetadata, DisabledReason, DisplayDimensionTarget,
    EditorMutation, EditorProblemCategory, EditorProblemMetadata, EditorProblemScope,
    EditorProblemTarget, FeatureAuthoringCornerBinding, FeatureAuthoringPointerDownOutcome,
    FeatureAuthoringPreview, FeatureAuthoringPreviewMetadata, FeatureAuthoringPreviewToken,
    FeatureAuthoringTransaction, GeometryRoleSelectionState, LifecycleDto, LifecycleStatus,
    MeasurementPublication, MutationOutcome, ProblemsDto, ProjectedDragRejectionStage,
    ProjectedDragWorkEvidence, RecordedComputedFeatureTransition, ReplayAction, RestoreCheckpoint,
    RetainedEditorCoordinator, SelectedCurvePropertyMetadata, display_dimension_target,
};
pub use curve_controls::{
    SceneCurveControl, SceneCurveControlGripGeometry, SceneCurveControlGuide,
    SceneCurveControlGuideKind, SceneCurveControlHit, SceneCurveControlInteraction,
    SceneCurveControlRail, SceneCurveControlRole,
};
pub use feature_authoring::{
    FeatureAuthoringCandidate, FeatureAuthoringCornerPreview, FeatureAuthoringGuidance,
    FeatureAuthoringOptions, FeatureAuthoringOutcome, FeatureAuthoringPick, FeatureAuthoringStage,
    FeatureAuthoringState, FeatureAuthoringTool, FeatureAuthoringWarning,
    FeatureAuthoringWarningKind,
};
pub use geometry_tools::{GeometryToolFamily, GeometryToolVariant};
pub use geosolve_sketch::SketchAcceptedDocumentRedundancy;
pub use geosolve_sketch_features::{
    ComputedCircularArc, ComputedConstructionFragment, ComputedConstructionFragmentId,
    ComputedConstructionFragmentProvenance, ComputedCornerRef, ComputedEdge, ComputedEdgeGeometry,
    ComputedEdgeId, ComputedEdgeProvenance, ComputedEvaluationAllocator,
    ComputedEvaluationAllocatorHighWater, ComputedEvaluationRevision, ComputedFeature,
    ComputedFeatureAllocatorHighWater, ComputedFeatureCornerId, ComputedFeatureDefinition,
    ComputedFeatureDocument, ComputedFeatureDocumentDigest, ComputedFeatureDocumentError,
    ComputedFeatureDocumentId, ComputedFeatureDocumentIdentity, ComputedFeatureEvaluation,
    ComputedFeatureEvaluationInput, ComputedFeatureEvaluationPolicy,
    ComputedFeatureEvaluationState, ComputedFeatureFailure, ComputedFeatureId,
    ComputedFeatureLifecycleHighWater, ComputedFeatureRevision, ComputedFeatureSnapshot,
    ComputedFilletContact, ComputedFilletCorner, ComputedFilletParentIndex, ComputedFilletSet,
    ComputedSourceInterval, NativeCurveSpanSource, NewComputedFilletCorner,
};
pub use inference::*;
use std::cmp::Ordering;

use geosolve_sketch::{
    ContactDomain, ContactNeighborhood, CurveDefinition, CurveId, CurveSpan, DesignPointId,
    DesignScalarId, DocumentAngleOrientation, DocumentArcSweep, DocumentBSplineForm,
    DocumentCenterRef, DocumentConstraintId, DocumentContactSeed, DocumentCurveContinuity,
    DocumentCurveControlId, DocumentCurveCurvatureRelation, DocumentCurveNormalSide,
    DocumentCurveSpanRef, DocumentDimensionId, DocumentDimensionMode, DocumentDirectionSense,
    DocumentEndpointRef, DocumentHyperbolaBranch, DocumentObjectId, FeatureEndpoint, GeometryRole,
    MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT, PreparedSketchInput, RetainedSketchDocumentSession,
    ScalarDomain, ScalarUnit, SketchDatum, SketchDesignIdentity, SketchDocument,
    TangentOrientation,
};
use thiserror::Error;

const MAX_TESSELLATION_DEPTH: u8 = 16;
// Seed non-linear spans before adaptive refinement so an inflection whose midpoint
// lies on its endpoint chord cannot disappear from rendering or hit testing.
const MIN_CURVED_TESSELLATION_DEPTH: u8 = 3;
// Small Fillet sweeps still need enough chords to read as arcs at ordinary zoom.
const MIN_COMPUTED_ARC_SEGMENTS: u16 = 8;
// Construction previews use model-space sampling before a viewport is available.
const ADVANCED_CURVE_PREVIEW_SUBDIVISIONS: u16 = 64;
// Preserve distinct local contacts whose tessellation distances are close
// enough that choosing only the first chord would be presentation-order bias.
const CURVE_BRANCH_CANDIDATE_BAND_PIXELS: f64 = 1.0;
const CURVE_POINTER_REFINEMENT_STEPS: u8 = 12;

/// A finite position in presentation pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenPoint {
    pub x: f64,
    pub y: f64,
}

impl ScreenPoint {
    fn distance(self, other: Self) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }

    fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

fn screen_unit(x: f64, y: f64) -> Option<[f64; 2]> {
    let length = x.hypot(y);
    (length.is_finite() && length > 1.0e-9).then_some([x / length, y / length])
}

fn model_positions_bit_equal(first: [f64; 2], second: [f64; 2]) -> bool {
    first[0].to_bits() == second[0].to_bits() && first[1].to_bits() == second[1].to_bits()
}

/// Model-to-screen mapping supplied by the presentation layer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub screen_size: [f64; 2],
    pub model_center: [f64; 2],
    pub pixels_per_model_unit: f64,
}

impl Viewport {
    /// Validates and constructs a viewport.
    ///
    /// # Errors
    ///
    /// Returns [`EditorError::InvalidViewport`] for non-finite, non-positive sizes or scale.
    pub fn new(
        screen_size: [f64; 2],
        model_center: [f64; 2],
        pixels_per_model_unit: f64,
    ) -> Result<Self, EditorError> {
        let viewport = Self {
            screen_size,
            model_center,
            pixels_per_model_unit,
        };
        if !viewport.is_valid() {
            return Err(EditorError::InvalidViewport);
        }
        Ok(viewport)
    }

    pub(crate) fn is_valid(self) -> bool {
        self.screen_size
            .into_iter()
            .all(|value| value.is_finite() && value > 0.0)
            && self.model_center.into_iter().all(f64::is_finite)
            && self.pixels_per_model_unit.is_finite()
            && self.pixels_per_model_unit > 0.0
    }

    /// Converts a finite model point to presentation pixels with positive model Y upward.
    #[must_use]
    pub fn model_to_screen(self, point: [f64; 2]) -> ScreenPoint {
        ScreenPoint {
            x: self.screen_size[0] * 0.5
                + (point[0] - self.model_center[0]) * self.pixels_per_model_unit,
            y: self.screen_size[1] * 0.5
                - (point[1] - self.model_center[1]) * self.pixels_per_model_unit,
        }
    }

    /// Converts presentation pixels to model coordinates.
    #[must_use]
    pub fn screen_to_model(self, point: ScreenPoint) -> [f64; 2] {
        [
            self.model_center[0]
                + (point.x - self.screen_size[0] * 0.5) / self.pixels_per_model_unit,
            self.model_center[1]
                - (point.y - self.screen_size[1] * 0.5) / self.pixels_per_model_unit,
        ]
    }
}

/// Selectable identity understood by the headless editor.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SelectionItem {
    Point(DesignPointId),
    Curve(CurveSpan),
    Constraint(DocumentConstraintId),
    Dimension(DocumentDimensionId),
    /// One intrinsic immutable Cartesian sketch datum.
    Datum(SketchDatum),
    /// One persistent computed feature outside the sketch constraint graph.
    Feature(geosolve_sketch_features::ComputedFeatureId),
    /// One persistent corner within a computed Fillet set.
    FeatureCorner(geosolve_sketch_features::ComputedCornerRef),
}

impl SelectionItem {
    /// Returns the owning persistent sketch object, if this is native sketch state.
    #[must_use]
    pub const fn object(self) -> Option<DocumentObjectId> {
        match self {
            Self::Point(id) => Some(DocumentObjectId::Point(id)),
            Self::Curve(span) => Some(DocumentObjectId::Curve(span.curve)),
            Self::Constraint(id) => Some(DocumentObjectId::Constraint(id)),
            Self::Dimension(id) => Some(DocumentObjectId::Dimension(id)),
            Self::Datum(_) | Self::Feature(_) | Self::FeatureCorner(_) => None,
        }
    }
}

/// Geometry families admitted by ordinary canvas interaction.
///
/// This is editor session state rather than persisted sketch state. Construction
/// geometry remains fully solver-active regardless of the selected scope.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GeometryPickScope {
    #[default]
    All,
    Profile,
    Construction,
}

/// Independent session-local visibility for persistent guides and computed
/// source portions discarded by Fillets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeometryVisibility {
    pub explicit_construction: bool,
    pub implicit_construction: bool,
    pub reference_geometry: bool,
}

impl Default for GeometryVisibility {
    fn default() -> Self {
        Self {
            explicit_construction: true,
            implicit_construction: true,
            reference_geometry: true,
        }
    }
}

/// One intrinsic Cartesian datum projected into the current finite viewport.
///
/// Axis endpoints are screen-space clipping representatives only. The semantic
/// datum remains an infinite line through `model_origin` in `model_direction`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneDatum {
    pub datum: SketchDatum,
    pub model_origin: [f64; 2],
    pub model_direction: Option<[f64; 2]>,
    pub screen_start: ScreenPoint,
    pub screen_end: ScreenPoint,
}

impl SceneDatum {
    /// Returns whether this datum has a painted representative in the supplied viewport.
    ///
    /// Picking and presentation adapters share this boundary so a datum just outside the
    /// mapped sketch plane cannot expose an invisible hit surface.
    #[must_use]
    pub fn is_visible_in_viewport(self, viewport: Viewport) -> bool {
        if !viewport.is_valid() {
            return false;
        }
        match self.datum {
            SketchDatum::Origin => {
                self.screen_start.x.is_finite()
                    && self.screen_start.y.is_finite()
                    && (0.0..=viewport.screen_size[0]).contains(&self.screen_start.x)
                    && (0.0..=viewport.screen_size[1]).contains(&self.screen_start.y)
            }
            SketchDatum::XAxis => {
                self.screen_start.y.is_finite()
                    && (0.0..=viewport.screen_size[1]).contains(&self.screen_start.y)
            }
            SketchDatum::YAxis => {
                self.screen_start.x.is_finite()
                    && (0.0..=viewport.screen_size[0]).contains(&self.screen_start.x)
            }
        }
    }
}

fn scene_datums(viewport: Viewport) -> Vec<SceneDatum> {
    let origin = viewport.model_to_screen([0.0, 0.0]);
    vec![
        SceneDatum {
            datum: SketchDatum::Origin,
            model_origin: [0.0, 0.0],
            model_direction: None,
            screen_start: origin,
            screen_end: origin,
        },
        SceneDatum {
            datum: SketchDatum::XAxis,
            model_origin: [0.0, 0.0],
            model_direction: Some([1.0, 0.0]),
            screen_start: ScreenPoint {
                x: 0.0,
                y: origin.y,
            },
            screen_end: ScreenPoint {
                x: viewport.screen_size[0],
                y: origin.y,
            },
        },
        SceneDatum {
            datum: SketchDatum::YAxis,
            model_origin: [0.0, 0.0],
            model_direction: Some([0.0, 1.0]),
            screen_start: ScreenPoint {
                x: origin.x,
                y: viewport.screen_size[1],
            },
            screen_end: ScreenPoint {
                x: origin.x,
                y: 0.0,
            },
        },
    ]
}

/// Complete headless geometry filtering policy used consistently by hover,
/// selection, drag ownership, snapping and authoring.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GeometryInteractionPolicy {
    pub scope: GeometryPickScope,
    pub visibility: GeometryVisibility,
}

/// Curve-role incidence used to filter persistent points without assigning a
/// persistent role to the point itself.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ScenePointRoleIncidence {
    pub profile: bool,
    pub construction: bool,
}

impl ScenePointRoleIncidence {
    const fn preferred_role(self, scope: GeometryPickScope) -> GeometryRole {
        match (scope, self.profile) {
            (GeometryPickScope::All | GeometryPickScope::Profile, true) => GeometryRole::Profile,
            (GeometryPickScope::All | GeometryPickScope::Profile, false)
            | (GeometryPickScope::Construction, _) => GeometryRole::Construction,
        }
    }
}

/// Presentation origin for one native-source curve occurrence.
///
/// Every occurrence still resolves to `SceneCurve::span`; an implicit Fillet
/// fragment never becomes a new persistent selection identity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SceneCurveOrigin {
    Native,
    FilletDiscarded {
        fragment: ComputedConstructionFragmentId,
        source: NativeCurveSpanSource,
        interval: ComputedSourceInterval,
        provenance: ComputedConstructionFragmentProvenance,
    },
}

impl SceneCurveOrigin {
    #[must_use]
    pub const fn is_implicit_construction(self) -> bool {
        matches!(self, Self::FilletDiscarded { .. })
    }
}

/// One accepted point primitive for presentation and picking.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScenePoint {
    pub id: DesignPointId,
    pub model_position: [f64; 2],
    pub screen_position: ScreenPoint,
    pub role_incidence: ScenePointRoleIncidence,
}

impl ScenePoint {
    /// Whether this point is displayed under session-local construction
    /// visibility. Pick scope deliberately does not hide displayed geometry.
    #[must_use]
    pub fn is_visible(self, policy: GeometryInteractionPolicy) -> bool {
        self.role_incidence.profile
            || (self.role_incidence.construction && policy.visibility.explicit_construction)
    }

    /// Whether this point may own interaction under the complete headless
    /// geometry policy. Hosts may keep a non-interactive point painted.
    #[must_use]
    pub fn is_interactive(self, policy: GeometryInteractionPolicy) -> bool {
        match policy.scope {
            GeometryPickScope::Profile => self.role_incidence.profile,
            GeometryPickScope::Construction => {
                self.role_incidence.construction && policy.visibility.explicit_construction
            }
            GeometryPickScope::All => {
                self.role_incidence.profile
                    || (self.role_incidence.construction && policy.visibility.explicit_construction)
            }
        }
    }

    fn is_pickable(self, policy: GeometryInteractionPolicy) -> bool {
        self.is_interactive(policy)
    }
}

/// One accepted semantic curve span represented by a display polyline.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneCurve {
    pub span: CurveSpan,
    /// Whether the persistent span still exists in retained design and may be
    /// captured by a new construction. Accepted-but-removed geometry remains
    /// paintable/pickable without becoming a stale inferred operand.
    pub authoring_eligible: bool,
    /// Whether this semantic span is a genuine affine line/polyline span.
    ///
    /// Presentation consumers must not infer this from tessellation chords.
    pub affine: bool,
    /// Exact persistent contact topology used by native drafting inference.
    /// Supporting-line contacts remain an explicit authoring choice and are
    /// therefore never selected by ordinary on-painted-curve inference.
    pub contact_domain: ContactDomain,
    /// Effective canvas/profile role for this visible occurrence.
    pub role: GeometryRole,
    /// Persistent role of the complete native source curve.
    pub source_role: GeometryRole,
    pub origin: SceneCurveOrigin,
    pub screen_polyline: Vec<ScreenPoint>,
    /// Curve parameters paired one-to-one with [`Self::screen_polyline`].
    pub screen_parameters: Vec<f64>,
    /// Optional semantic point moved when this visible curve is dragged.
    ///
    /// Selection remains curve-based. The handle only defines gesture ownership,
    /// so presentation adapters do not need to infer a circular curve's center.
    pub drag_handle_point: Option<DesignPointId>,
}

impl SceneCurve {
    /// Whether this curve occurrence is displayed under session-local
    /// construction visibility. Pick scope deliberately does not hide it.
    #[must_use]
    pub fn is_visible(&self, policy: GeometryInteractionPolicy) -> bool {
        match self.role {
            GeometryRole::Profile => true,
            GeometryRole::Construction if self.origin.is_implicit_construction() => {
                policy.visibility.implicit_construction
            }
            GeometryRole::Construction => policy.visibility.explicit_construction,
        }
    }

    /// Whether this native occurrence may own interaction under the complete
    /// headless geometry policy. Hosts may keep a non-interactive curve painted.
    #[must_use]
    pub fn is_interactive(&self, policy: GeometryInteractionPolicy) -> bool {
        self.is_visible(policy) && role_participates(self.role, policy.scope)
    }

    fn is_pickable(&self, policy: GeometryInteractionPolicy) -> bool {
        self.is_interactive(policy)
    }
}

/// One evaluation-local computed curve with stable feature/corner selection.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneComputedCurve {
    pub edge: geosolve_sketch_features::ComputedEdgeId,
    pub owner: geosolve_sketch_features::ComputedCornerRef,
    /// Effective role inherited from the computed feature's native sources.
    pub role: GeometryRole,
    pub center: [f64; 2],
    pub radius: f64,
    pub start_angle: f64,
    pub end_angle: f64,
    pub sweep: DocumentArcSweep,
    pub contacts: [geosolve_sketch_features::ComputedFilletContact; 2],
    pub screen_polyline: Vec<ScreenPoint>,
    /// Optional headless radius continuation rail for direct manipulation.
    ///
    /// The rail is frozen at pointer down. Presentation code may draw it, but
    /// the editor remains the sole owner of projecting pointer motion onto it.
    pub radius_rail: Option<SceneFilletRadiusRail>,
}

impl SceneComputedCurve {
    /// Whether this generated arc is displayed under session-local construction
    /// visibility. Pick scope deliberately does not hide it.
    #[must_use]
    pub fn is_visible(&self, policy: GeometryInteractionPolicy) -> bool {
        self.role == GeometryRole::Profile || policy.visibility.explicit_construction
    }

    /// Whether this computed result may own interaction under the complete
    /// headless geometry policy. Hosts use this to suppress affordances while
    /// retaining scope-independent geometry rendering.
    #[must_use]
    pub fn is_interactive(&self, policy: GeometryInteractionPolicy) -> bool {
        self.is_visible(policy) && role_participates(self.role, policy.scope)
    }

    fn is_pickable(&self, policy: GeometryInteractionPolicy) -> bool {
        self.is_interactive(policy)
    }
}

const fn role_participates(role: GeometryRole, scope: GeometryPickScope) -> bool {
    match scope {
        GeometryPickScope::All => true,
        GeometryPickScope::Profile => matches!(role, GeometryRole::Profile),
        GeometryPickScope::Construction => matches!(role, GeometryRole::Construction),
    }
}

/// One finite one-dimensional Fillet-radius continuation rail.
///
/// `model_derivative` is `d(center) / d(radius)` on the selected absolute
/// branch. Pointer motion is projected onto this vector, so motion orthogonal
/// to the rail cannot change radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneFilletRadiusRail {
    pub owner: geosolve_sketch_features::ComputedCornerRef,
    pub model_center: [f64; 2],
    pub model_grip: [f64; 2],
    pub model_derivative: [f64; 2],
    pub screen_center: ScreenPoint,
    pub screen_grip: ScreenPoint,
    pub screen_rail_start: ScreenPoint,
    pub screen_rail_end: ScreenPoint,
}

impl SceneFilletRadiusRail {
    fn is_valid(self) -> bool {
        let derivative_norm_squared = self.model_derivative[0]
            .mul_add(self.model_derivative[0], self.model_derivative[1].powi(2));
        self.model_center.into_iter().all(f64::is_finite)
            && self.model_grip.into_iter().all(f64::is_finite)
            && self.model_derivative.into_iter().all(f64::is_finite)
            && self.screen_center.is_finite()
            && self.screen_grip.is_finite()
            && self.screen_rail_start.is_finite()
            && self.screen_rail_end.is_finite()
            && derivative_norm_squared.is_finite()
            && derivative_norm_squared > 0.0
    }
}

/// One named contact handle for a computed Fillet corner.
///
/// The handle identifies only an already-evaluated parent and contact. It does
/// not select a different root or infer any feature-domain branch state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneFilletContactHandle {
    pub owner: geosolve_sketch_features::ComputedCornerRef,
    pub parent: ComputedFilletParentIndex,
    pub source: NativeCurveSpanSource,
    pub parameter: f64,
    pub model_position: [f64; 2],
    pub screen_position: ScreenPoint,
}

impl SceneFilletContactHandle {
    fn is_valid(self) -> bool {
        self.parameter.is_finite()
            && self.model_position.into_iter().all(f64::is_finite)
            && self.screen_position.is_finite()
    }
}

/// Stable geometric reason that a Fillet continuation sample could not advance.
///
/// This classifies an already-typed feature/coordinator failure; presentation
/// code never infers the category by parsing [`ComputedFilletContinuationLimit::message`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComputedFilletContinuationLimitKind {
    BranchFold,
    DomainBoundary,
    OffsetSingularity,
    LossOfRegularity,
    AmbiguousLocalRoot,
    WorkStopped,
}

/// Human-readable metadata paired with one typed Fillet continuation limit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComputedFilletContinuationLimit {
    pub kind: ComputedFilletContinuationLimitKind,
    pub message: String,
}

impl ComputedFilletContinuationLimit {
    fn is_valid(&self) -> bool {
        !self.message.trim().is_empty()
    }
}

/// Exact rejected radius/contact sample associated with a continuation limit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ComputedFilletInteractionSample {
    Radius(f64),
    Contact {
        parent: ComputedFilletParentIndex,
        source: NativeCurveSpanSource,
        parameter: f64,
    },
}

impl ComputedFilletInteractionSample {
    fn is_valid(self) -> bool {
        match self {
            Self::Radius(radius) => radius.is_finite() && radius > 0.0,
            Self::Contact { parameter, .. } => parameter.is_finite(),
        }
    }
}

/// Exact active-gesture status for a rejected Fillet continuation sample.
///
/// The previously accepted `Current` preview remains separate scene geometry;
/// this DTO describes why only the newer requested sample was rejected.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedFilletContinuationStatus {
    pub expected: geosolve_sketch_features::ComputedFeatureEvaluationInput,
    pub owner: geosolve_sketch_features::ComputedCornerRef,
    pub sample: ComputedFilletInteractionSample,
    pub limit: ComputedFilletContinuationLimit,
}

impl ComputedFilletContinuationStatus {
    fn is_valid(&self) -> bool {
        self.sample.is_valid() && self.limit.is_valid()
    }
}

/// Stable presentation-neutral identity for one explicit Fillet action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SceneFilletActionId {
    ReverseFirstRetainedDirection,
    ReverseSecondRetainedDirection,
    ComplementaryArc,
    /// Selects an independently validated normal-side pair. The semantic pair
    /// remains stable when a different local alternative becomes unavailable;
    /// it is never an ordinal in the currently visible list.
    LocalAlternative {
        first: DocumentCurveNormalSide,
        second: DocumentCurveNormalSide,
    },
}

/// Applicability supplied by the coordinator for one Fillet action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SceneFilletActionAvailability {
    Applicable,
    Disabled { reason: String },
}

/// Finite presentation geometry for one directional Fillet action.
///
/// `model_anchor` and `model_direction` retain presentation-independent
/// geometry for non-browser consumers. `screen_start` and `screen_end` are the
/// exact scene projection used by the workbench, so presentation adapters do
/// not need to reconstruct tangent or retained-direction policy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneFilletActionControlGeometry {
    pub model_anchor: [f64; 2],
    pub model_direction: [f64; 2],
    pub screen_start: ScreenPoint,
    pub screen_end: ScreenPoint,
}

impl SceneFilletActionControlGeometry {
    fn is_valid(self, viewport: Viewport) -> bool {
        let model_norm_squared = self.model_direction[0]
            .mul_add(self.model_direction[0], self.model_direction[1].powi(2));
        let screen_length = self.screen_start.distance(self.screen_end);
        if !(self.model_anchor.into_iter().all(f64::is_finite)
            && self.model_direction.into_iter().all(f64::is_finite)
            && model_norm_squared.is_finite()
            && model_norm_squared > 0.0
            && self.screen_start.is_finite()
            && self.screen_end.is_finite()
            && screen_length.is_finite()
            && screen_length > 0.0)
        {
            return false;
        }
        let projected_anchor = viewport.model_to_screen(self.model_anchor);
        if !screen_points_match(projected_anchor, self.screen_start) {
            return false;
        }
        let model_end = viewport.screen_to_model(self.screen_end);
        let model_delta = [
            model_end[0] - self.model_anchor[0],
            model_end[1] - self.model_anchor[1],
        ];
        let model_length = model_delta[0].hypot(model_delta[1]);
        if !model_length.is_finite() || model_length <= 0.0 {
            return false;
        }
        let direction_length = model_norm_squared.sqrt();
        let alignment = (model_delta[0] / model_length).mul_add(
            self.model_direction[0] / direction_length,
            (model_delta[1] / model_length) * (self.model_direction[1] / direction_length),
        );
        alignment.is_finite() && (1.0 - alignment).abs() <= 1.0e-10
    }
}

/// Paired model-space and exact scene-projected geometry for one non-authoritative
/// Fillet branch alternative.
///
/// Presentation adapters render `screen_polyline`; the editor independently uses
/// `model_polyline` for hit validation. Both arrays describe the same ordered
/// samples and are checked against the scene viewport before admission.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneFilletAlternativeGeometry {
    pub model_polyline: Vec<[f64; 2]>,
    pub screen_polyline: Vec<ScreenPoint>,
}

impl SceneFilletAlternativeGeometry {
    fn is_valid(&self, viewport: Viewport) -> bool {
        self.model_polyline.len() >= 2
            && self.model_polyline.len() == self.screen_polyline.len()
            && self
                .model_polyline
                .iter()
                .zip(&self.screen_polyline)
                .all(|(model, screen)| {
                    model.iter().copied().all(f64::is_finite)
                        && screen.is_finite()
                        && screen_points_match(viewport.model_to_screen(*model), *screen)
                })
    }
}

/// Exact scene-stamped identity for one explicit Fillet action.
///
/// Owner/action pairs alone are not actionable: the complete computed input
/// prevents a stale canvas or accessibility control from targeting a newer
/// feature configuration that happens to reuse the same persistent IDs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneFilletActionTarget {
    pub expected: geosolve_sketch_features::ComputedFeatureEvaluationInput,
    pub owner: geosolve_sketch_features::ComputedCornerRef,
    pub action: SceneFilletActionId,
}

/// Presentation-neutral source for Fillet branch preview and activation.
///
/// A painted canvas target is only a hint and must independently agree with a
/// model-space proximity hit. An accessible target has no pointer proximity,
/// but must carry the exact current scene stamp and resolve to the same
/// applicable action through the shared validator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SceneFilletActionInput {
    Canvas {
        position: ScreenPoint,
        painted: Option<SceneFilletActionTarget>,
    },
    Accessible(SceneFilletActionTarget),
}

/// One stable Fillet action and optional non-authoritative alternative arc.
///
/// The editor validates owner identity and finite presentation geometry only.
/// The coordinator remains responsible for semantic applicability and for the
/// complete absolute replacement-corner intent behind the action ID.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneFilletAction {
    pub id: SceneFilletActionId,
    pub owner: geosolve_sketch_features::ComputedCornerRef,
    pub label: String,
    pub availability: SceneFilletActionAvailability,
    /// Optional headless control geometry, such as a retained-direction arrow.
    pub control_geometry: Option<SceneFilletActionControlGeometry>,
    pub dashed_alternative_arc: Option<SceneFilletAlternativeGeometry>,
}

impl SceneFilletAction {
    fn is_valid(
        &self,
        owner: geosolve_sketch_features::ComputedCornerRef,
        viewport: Viewport,
    ) -> bool {
        if self.owner != owner || self.label.trim().is_empty() {
            return false;
        }
        if matches!(
            &self.availability,
            SceneFilletActionAvailability::Disabled { reason } if reason.trim().is_empty()
        ) {
            return false;
        }
        if self
            .control_geometry
            .is_some_and(|geometry| !geometry.is_valid(viewport))
        {
            return false;
        }
        self.dashed_alternative_arc
            .as_ref()
            .is_none_or(|geometry| geometry.is_valid(viewport))
    }
}

/// Complete visible direct-manipulation affordances for one Fillet corner.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneFilletCornerAffordances {
    pub owner: geosolve_sketch_features::ComputedCornerRef,
    /// Sorted unique corner owners changed by this shared-radius rail.
    ///
    /// The list always contains `owner`; presentation adapters may therefore
    /// highlight every arc in the `FilletSet` without reconstructing ownership.
    pub affected_owners: Vec<geosolve_sketch_features::ComputedCornerRef>,
    pub radius_rail: SceneFilletRadiusRail,
    pub contacts: [SceneFilletContactHandle; 2],
    pub actions: Vec<SceneFilletAction>,
    /// Typed limit for the exact active gesture sample, while the last current
    /// radius/contact preview remains solid.
    pub continuation_status: Option<ComputedFilletContinuationStatus>,
}

/// One deterministic Fillet-aware hit.
///
/// The visible radius grip/spoke/rail/arc wins over native accepted geometry.
/// Endpoint contact metadata is not part of the canvas hit surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SceneFilletHit {
    Radius {
        owner: geosolve_sketch_features::ComputedCornerRef,
        distance_pixels: f64,
    },
    Native(Hit),
}

impl SceneFilletHit {
    /// Persistent selection item represented by this hit.
    #[must_use]
    pub const fn item(self) -> SelectionItem {
        match self {
            Self::Radius { owner, .. } => SelectionItem::FeatureCorner(owner),
            Self::Native(hit) => hit.item,
        }
    }
}

/// Exact constructor-owned scene semantics that may participate in drafting
/// inference publication.
///
/// `EditorScene` remains an ergonomic presentation DTO with public fields, so
/// a host may alter a detached scene for rendering or compatibility behavior.
/// Those alterations must not retain (or manufacture) retained-session
/// publication authority. Keeping the constructor result behind this private
/// seal lets the editor compare exact inference-visible values without relying
/// on a caller-maintained dirty bit or a collision-prone digest.
#[derive(Clone, Debug, PartialEq)]
struct DraftInferenceSceneSeal {
    accepted_revision: u64,
    design_identity: SketchDesignIdentity,
    viewport: Viewport,
    curves: Vec<SceneCurve>,
    datums: Vec<SceneDatum>,
    constraint_entries: Vec<SceneConstraintEntry>,
    construction_snap_points: Vec<ScenePoint>,
}

/// Exact candidate-scene values retained beside the pointer-down stamp for one
/// already-live selected-curve control gesture.
///
/// A prepared control edit advances the candidate design and accepted revision
/// before compare-and-swap publication. The scene must therefore keep those
/// candidate identities truthful while separately remembering which durable
/// pointer-down interaction it may complete. This private seal prevents a host
/// from changing the candidate control surface after that origin was attached.
#[derive(Clone, Debug, PartialEq)]
struct CurveControlInteractionOrigin {
    accepted_revision: u64,
    design_identity: SketchDesignIdentity,
    request_id: u64,
    model_position: [f64; 2],
    candidate_revision: u64,
    candidate_design_identity: SketchDesignIdentity,
    viewport: Viewport,
    curve_controls: Vec<SceneCurveControl>,
    curve_control_guides: Vec<SceneCurveControlGuide>,
}

impl CurveControlInteractionOrigin {
    fn capture(
        scene: &EditorScene,
        accepted_revision: u64,
        design_identity: SketchDesignIdentity,
        request_id: u64,
        model_position: [f64; 2],
    ) -> Self {
        Self {
            accepted_revision,
            design_identity,
            request_id,
            model_position,
            candidate_revision: scene.accepted_revision,
            candidate_design_identity: scene.design_identity,
            viewport: scene.viewport,
            curve_controls: scene.curve_controls.clone(),
            curve_control_guides: scene.curve_control_guides.clone(),
        }
    }

    fn matches(
        &self,
        scene: &EditorScene,
        accepted_revision: u64,
        design_identity: SketchDesignIdentity,
        viewport: Viewport,
        control: DocumentCurveControlId,
        owner: CurveSpan,
    ) -> bool {
        self.accepted_revision == accepted_revision
            && self.design_identity == design_identity
            && self.candidate_revision == scene.accepted_revision
            && self.candidate_design_identity == scene.design_identity
            && self.viewport == viewport
            && scene.viewport == viewport
            && self.curve_controls == scene.curve_controls
            && self.curve_control_guides == scene.curve_control_guides
            && scene.curve_controls.iter().any(|candidate| {
                candidate.id == control && candidate.owner == owner && candidate.is_editable()
            })
    }

    fn matches_request(&self, last_valid_request: Option<(u64, [f64; 2])>) -> bool {
        last_valid_request.is_some_and(|(request_id, model_position)| {
            self.request_id == request_id
                && model_positions_bit_equal(self.model_position, model_position)
        })
    }
}

impl DraftInferenceSceneSeal {
    fn capture(scene: &EditorScene) -> Self {
        Self {
            accepted_revision: scene.accepted_revision,
            design_identity: scene.design_identity,
            viewport: scene.viewport,
            curves: scene.curves.clone(),
            datums: scene.datums.clone(),
            constraint_entries: scene.constraint_entries.clone(),
            construction_snap_points: scene.construction_snap_points.clone(),
        }
    }

    fn matches(&self, scene: &EditorScene) -> bool {
        self.accepted_revision == scene.accepted_revision
            && self.design_identity == scene.design_identity
            && self.viewport == scene.viewport
            && self.curves == scene.curves
            && self.datums == scene.datums
            && self.constraint_entries == scene.constraint_entries
            && self.construction_snap_points == scene.construction_snap_points
    }
}

/// Deterministic presentation-neutral scene derived from one accepted revision.
#[derive(Clone, Debug, PartialEq)]
pub struct EditorScene {
    pub accepted_revision: u64,
    pub design_identity: SketchDesignIdentity,
    /// Exact retained-session input that certified the accepted geometry.
    ///
    /// Legacy scene constructors leave this absent. Such scenes remain valid
    /// for rendering, picking, and ordinary construction, but cannot authorize
    /// an inferred construction plan.
    prepared_input: Option<PreparedSketchInput>,
    /// Private constructor-owned copy of every scene value consumed by drafting
    /// inference. Public presentation-field mutation invalidates publication
    /// authority instead of silently changing authenticated semantics.
    draft_inference_seal: Option<DraftInferenceSceneSeal>,
    pub viewport: Viewport,
    pub points: Vec<ScenePoint>,
    pub curves: Vec<SceneCurve>,
    /// Intrinsic immutable reference geometry for the Cartesian sketch plane.
    pub datums: Vec<SceneDatum>,
    /// Generated Fillet arcs. Source replacement fragments remain native
    /// [`SceneCurve`] values so native span selection and dragging stay intact.
    pub computed_curves: Vec<SceneComputedCurve>,
    /// Selected-only transient grips owned by one native curve.
    ///
    /// These are recomputed from accepted geometry and editor selection. They
    /// are never persistent points, constraint operands, or snapping anchors.
    pub curve_controls: Vec<SceneCurveControl>,
    /// Exact control polygon, size rail, spoke and axis paint geometry paired
    /// with [`Self::curve_controls`].
    pub curve_control_guides: Vec<SceneCurveControlGuide>,
    pub feature_identity: Option<geosolve_sketch_features::ComputedFeatureDocumentIdentity>,
    pub computed_input: Option<geosolve_sketch_features::ComputedFeatureEvaluationInput>,
    fillet_interaction_origin: Option<geosolve_sketch_features::ComputedFeatureEvaluationInput>,
    curve_control_interaction_origin: Option<CurveControlInteractionOrigin>,
    /// Explicit direct-manipulation affordances supplied for current Fillet corners.
    pub fillet_affordances: Vec<SceneFilletCornerAffordances>,
    /// Typed per-corner continuation limits. These remain available even when a
    /// fold/singularity prevents construction of a radius rail affordance.
    pub computed_fillet_continuation_statuses: Vec<ComputedFilletContinuationStatus>,
    /// Accepted, geometry-derived constraint and dimension presentation.
    pub annotations: Vec<SceneAnnotation>,
    /// Whether ordinary contextual constraint marks are part of this scene's
    /// visible and interactive presentation surface.
    ///
    /// This is presentation-only scene policy. Keeping it on the headless DTO
    /// ensures painting, hover and pointer-down all resolve the same marks.
    pub show_all_constraint_annotations: bool,
    /// Complete persistent constraint list for presentation surfaces such as
    /// trees and inspectors. This remains populated even when a constraint has
    /// no drawable annotation anchor.
    pub constraint_entries: Vec<SceneConstraintEntry>,
    construction_snap_points: Vec<ScenePoint>,
    /// Exact accepted-domain evaluator used after screen-space tessellation has
    /// selected a semantic parameter.  This keeps preview correction on the
    /// owning sketch equations instead of treating display chords as geometry.
    accepted_document: SketchDocument,
}

impl EditorScene {
    /// Resolves exact driving/reference values into compact labels and shared hit bounds.
    pub fn update_annotation_values(
        &mut self,
        accepted: &geosolve_sketch::SketchAcceptedDocumentState,
    ) -> bool {
        if accepted.document().id() != self.accepted_document.id()
            || accepted.identity().revision().get() != self.accepted_revision
            || accepted.document() != &self.accepted_document
        {
            return false;
        }
        annotations::update_dimension_values(&mut self.annotations, accepted);
        true
    }

    /// Applies editor-owned presentation layout without changing accepted geometry authority.
    pub fn apply_annotation_layout(&mut self, layout: &AnnotationLayoutState) {
        // Recompose from geometry-derived automatic positions so repeated host
        // rebuilds are idempotent. Runtime reference values are then copied
        // back before manual placements and collision resolution are applied.
        let previous = std::mem::take(&mut self.annotations);
        let mut annotations = annotations::build_annotations(
            &self.accepted_document,
            &self.points,
            &self.curves,
            self.viewport,
        );
        for annotation in &mut annotations {
            if let Some(prior) = previous
                .iter()
                .find(|prior| prior.item == annotation.item && prior.source == annotation.source)
            {
                annotation.visible_text.clone_from(&prior.visible_text);
                annotation
                    .accessible_label
                    .clone_from(&prior.accessible_label);
                annotation.reference = prior.reference;
                annotation.refresh_label_bounds();
            }
        }
        annotations::apply_layout(
            self.accepted_document.id(),
            &mut annotations,
            &self.points,
            &self.curves,
            self.viewport,
            layout,
        );
        self.annotations = annotations;
    }

    /// Sets the shared paint/pick visibility policy for contextual constraints.
    pub fn set_show_all_constraint_annotations(&mut self, show: bool) {
        self.show_all_constraint_annotations = show;
    }

    fn set_selected_curve_controls(&mut self, owner: Option<CurveSpan>) -> Result<(), EditorError> {
        self.curve_control_interaction_origin = None;
        self.curve_controls.clear();
        self.curve_control_guides.clear();
        let Some(owner) = owner else {
            return Ok(());
        };
        if !self
            .accepted_document
            .curve_spans(owner.curve)?
            .contains(&owner)
        {
            return Ok(());
        }
        if !self.curves.iter().any(|curve| {
            curve.span == owner
                && curve.authoring_eligible
                && matches!(
                    curve.origin,
                    SceneCurveOrigin::Native | SceneCurveOrigin::FilletDiscarded { .. }
                )
        }) {
            return Ok(());
        }
        let (controls, guides) = curve_controls::build_selected_curve_controls(
            &self.accepted_document,
            owner,
            self.viewport,
        )?;
        self.curve_controls = controls;
        self.curve_control_guides = guides;
        Ok(())
    }

    /// Resolves a visible selected-curve grip or its owned rail/spoke through
    /// the same finite geometry published for rendering.
    #[must_use]
    pub fn curve_control_hit_test(
        &self,
        position: ScreenPoint,
        tolerance: PickTolerance,
    ) -> Option<SceneCurveControlHit> {
        self.curve_control_hit_test_with_policy(
            position,
            tolerance,
            GeometryInteractionPolicy::default(),
        )
    }

    /// Policy-aware counterpart of [`Self::curve_control_hit_test`].
    #[must_use]
    pub fn curve_control_hit_test_with_policy(
        &self,
        position: ScreenPoint,
        tolerance: PickTolerance,
        policy: GeometryInteractionPolicy,
    ) -> Option<SceneCurveControlHit> {
        curve_controls::curve_control_hit_test(
            &self.curve_controls,
            &self.curve_control_guides,
            &self.curves,
            position,
            tolerance,
            policy,
        )
    }

    /// Builds screen-space primitives only through public immutable sketch evaluation.
    ///
    /// # Errors
    ///
    /// Returns a typed public curve-evaluation error or rejects invalid scene options.
    pub fn from_accepted(
        accepted_revision: u64,
        design_identity: SketchDesignIdentity,
        document: &SketchDocument,
        viewport: Viewport,
        chord_tolerance_pixels: f64,
    ) -> Result<Self, EditorError> {
        if !chord_tolerance_pixels.is_finite() || chord_tolerance_pixels <= 0.0 {
            return Err(EditorError::InvalidTolerance);
        }
        Self::from_accepted_with_snap_filter(
            accepted_revision,
            design_identity,
            document,
            None,
            viewport,
            chord_tolerance_pixels,
        )
    }

    /// Builds the accepted visible scene while publishing the current retained
    /// design's constraint entries and restricting construction snaps to design
    /// identities that still exist.
    ///
    /// Picking, curve geometry and annotation coordinates continue to use only
    /// accepted geometry. Constraint entries require no solved coordinates, so
    /// they follow the supplied design and keep rejected intent visible without
    /// giving rejected geometry presentation authority.
    ///
    /// # Errors
    ///
    /// Returns a typed public curve-evaluation error or rejects invalid scene options.
    pub fn from_accepted_for_design(
        accepted_revision: u64,
        design_identity: SketchDesignIdentity,
        accepted_document: &SketchDocument,
        design_document: &SketchDocument,
        viewport: Viewport,
        chord_tolerance_pixels: f64,
    ) -> Result<Self, EditorError> {
        Self::from_accepted_with_snap_filter(
            accepted_revision,
            design_identity,
            accepted_document,
            Some(design_document),
            viewport,
            chord_tolerance_pixels,
        )
    }

    fn from_accepted_with_snap_filter(
        accepted_revision: u64,
        design_identity: SketchDesignIdentity,
        accepted_document: &SketchDocument,
        design_document: Option<&SketchDocument>,
        viewport: Viewport,
        chord_tolerance_pixels: f64,
    ) -> Result<Self, EditorError> {
        if !chord_tolerance_pixels.is_finite() || chord_tolerance_pixels <= 0.0 {
            return Err(EditorError::InvalidTolerance);
        }
        let point_roles = point_role_incidence(accepted_document);
        let points: Vec<_> = accepted_document
            .points()
            .iter()
            .map(|point| ScenePoint {
                id: point.id,
                model_position: point.position,
                screen_position: viewport.model_to_screen(point.position),
                role_incidence: point_roles.get(&point.id).copied().unwrap_or(
                    ScenePointRoleIncidence {
                        profile: true,
                        construction: false,
                    },
                ),
            })
            .collect();
        let construction_snap_points = points
            .iter()
            .copied()
            .filter(|point| design_document.is_none_or(|design| design.point(point.id).is_some()))
            .collect();
        let curves = build_native_scene_curves(
            accepted_document,
            design_document,
            viewport,
            chord_tolerance_pixels,
        )?;
        let annotations =
            annotations::build_annotations(accepted_document, &points, &curves, viewport);
        let constraint_entries =
            annotations::build_constraint_entries(design_document.unwrap_or(accepted_document));
        let mut scene = Self {
            accepted_revision,
            design_identity,
            prepared_input: None,
            draft_inference_seal: None,
            viewport,
            points,
            curves,
            datums: scene_datums(viewport),
            computed_curves: Vec::new(),
            curve_controls: Vec::new(),
            curve_control_guides: Vec::new(),
            feature_identity: None,
            computed_input: None,
            fillet_interaction_origin: None,
            curve_control_interaction_origin: None,
            fillet_affordances: Vec::new(),
            computed_fillet_continuation_statuses: Vec::new(),
            annotations,
            show_all_constraint_annotations: false,
            constraint_entries,
            construction_snap_points,
            accepted_document: accepted_document.clone(),
        };
        scene.refresh_draft_inference_seal();
        Ok(scene)
    }

    /// Binds this scene to the exact retained session that certified its accepted
    /// geometry.
    ///
    /// Drafting inference may still be presented by an unbound compatibility
    /// scene, but only a bound scene can emit an atomic inferred construction
    /// plan. This prevents a scene reconstructed from document/revision fields
    /// alone from acquiring publication authority.
    ///
    /// # Errors
    ///
    /// Rejects a session whose current accepted document, design filter, or
    /// lifecycle stamp differs from the scene, or when an inference-visible
    /// public scene field changed after trusted construction, without changing
    /// the scene. A later change to one of those fields revokes publication
    /// authority while leaving detached inference presentation available.
    /// Taking the retained session instead of a detached
    /// [`PreparedSketchInput`] is deliberate: identities and revisions alone
    /// cannot authenticate scene geometry supplied by a caller.
    pub fn with_retained_session(
        mut self,
        session: &RetainedSketchDocumentSession,
    ) -> Result<Self, EditorError> {
        let prepared_input = session
            .accepted_prepared_input()
            .ok_or(EditorError::StalePreparedSketchInput)?;
        let accepted = prepared_input
            .accepted_state_identity()
            .ok_or(EditorError::StalePreparedSketchInput)?;
        let accepted_state = session
            .accepted_state_for_current_input()
            .ok_or(EditorError::StalePreparedSketchInput)?;
        if !self.draft_inference_semantics_are_sealed()
            || prepared_input.design_identity() != self.design_identity
            || prepared_input.latest_attempt_identity().document() != self.accepted_document.id()
            || accepted.document() != self.accepted_document.id()
            || accepted.revision().get() != self.accepted_revision
            || accepted_state.identity() != accepted
            || accepted_state.document() != &self.accepted_document
            || !self.matches_design_filter(session.design_document())
        {
            return Err(EditorError::StalePreparedSketchInput);
        }
        self.prepared_input = Some(prepared_input);
        Ok(self)
    }

    fn refresh_draft_inference_seal(&mut self) {
        self.draft_inference_seal = Some(DraftInferenceSceneSeal::capture(self));
    }

    fn draft_inference_semantics_are_sealed(&self) -> bool {
        self.draft_inference_seal
            .as_ref()
            .is_some_and(|seal| seal.matches(self))
    }

    pub(crate) fn authenticated_prepared_input(&self) -> Option<PreparedSketchInput> {
        self.draft_inference_semantics_are_sealed()
            .then_some(self.prepared_input)
            .flatten()
    }

    pub(crate) fn set_curve_control_interaction_origin(
        &mut self,
        accepted_revision: u64,
        design_identity: SketchDesignIdentity,
        request_id: u64,
        model_position: [f64; 2],
    ) {
        self.prepared_input = None;
        self.draft_inference_seal = None;
        self.curve_control_interaction_origin = Some(CurveControlInteractionOrigin::capture(
            self,
            accepted_revision,
            design_identity,
            request_id,
            model_position,
        ));
    }

    fn accepts_curve_control_gesture(
        &self,
        accepted_revision: u64,
        design_identity: SketchDesignIdentity,
        viewport: Viewport,
        control: DocumentCurveControlId,
        owner: CurveSpan,
        last_valid_request: Option<(u64, [f64; 2])>,
    ) -> bool {
        let control_is_current = self.curve_controls.iter().any(|candidate| {
            candidate.id == control && candidate.owner == owner && candidate.is_editable()
        });
        let current_scene = self.accepted_revision == accepted_revision
            && self.design_identity == design_identity
            && self.viewport == viewport
            && control_is_current;
        self.curve_control_interaction_origin
            .as_ref()
            .map_or(current_scene, |origin| {
                origin.matches_request(last_valid_request)
                    && origin.matches(
                        self,
                        accepted_revision,
                        design_identity,
                        viewport,
                        control,
                        owner,
                    )
            })
    }

    fn matches_design_filter(&self, design: &SketchDocument) -> bool {
        let point_roles = point_role_incidence(&self.accepted_document);
        let expected_snap_points = self
            .accepted_document
            .points()
            .iter()
            .filter(|point| design.point(point.id).is_some())
            .map(|point| ScenePoint {
                id: point.id,
                model_position: point.position,
                screen_position: self.viewport.model_to_screen(point.position),
                role_incidence: point_roles.get(&point.id).copied().unwrap_or(
                    ScenePointRoleIncidence {
                        profile: true,
                        construction: false,
                    },
                ),
            })
            .collect::<Vec<_>>();
        self.constraint_entries == annotations::build_constraint_entries(design)
            && self.construction_snap_points == expected_snap_points
            && self.curves.iter().all(|curve| {
                curve.authoring_eligible
                    == design
                        .curve_spans(curve.span.curve)
                        .is_ok_and(|spans| spans.contains(&curve.span))
            })
    }

    /// Produces native semantic anchors for one drafting-inference sample.
    ///
    /// Only point identities and native curve occurrences that still exist in
    /// retained design participate. Generated Fillet arcs are intentionally
    /// absent; a discarded Fillet source fragment participates only through its
    /// mapped native [`CurveSpan`] and explicit origin metadata. Anchor count
    /// and tessellation-chord work are bounded before projection; exhaustion
    /// returns typed evidence and no partial anchor prefix.
    #[must_use]
    pub fn draft_inference_anchors(
        &self,
        pointer: ScreenPoint,
        limits: DraftInferenceLimits,
    ) -> DraftInferenceAnchorCollection {
        if !pointer.is_finite() {
            return DraftInferenceAnchorCollection::Complete {
                anchors: Vec::new(),
            };
        }
        if let Some(evidence) = draft_inference_scene_resource_limit(self, limits) {
            return DraftInferenceAnchorCollection::ResourceLimited(evidence);
        }

        let mut anchors = self
            .construction_snap_points
            .iter()
            .map(|point| DraftReferenceAnchor::PersistentPoint {
                point: point.id,
                model_position: point.model_position,
                role_incidence: point.role_incidence,
            })
            .collect::<Vec<_>>();
        let mut nonlinear_samples = Vec::new();

        for curve in self.curves.iter().filter(|curve| curve.authoring_eligible) {
            let samples =
                scene_curve_pointer_samples(curve, &self.accepted_document, self.viewport, pointer);
            if samples.is_empty() {
                continue;
            }
            let origin = match curve.origin {
                SceneCurveOrigin::Native => DraftReferenceOrigin::Native,
                SceneCurveOrigin::FilletDiscarded { .. } => DraftReferenceOrigin::FilletDiscarded,
            };
            if curve.affine {
                let sample = samples[0];
                let Some(contact) = draft_curve_contact(
                    &self.accepted_document,
                    curve.span,
                    curve.contact_domain,
                    sample.total_parameter,
                ) else {
                    continue;
                };
                let Some(affine_direction) = scene_curve_affine_direction(curve, self.viewport)
                else {
                    continue;
                };
                anchors.push(DraftReferenceAnchor::AffineSupport {
                    contact,
                    model_position: sample.model_position,
                    affine_direction,
                    role: curve.role,
                    source_role: curve.source_role,
                    origin,
                });
                if let Some(model_position) =
                    scene_curve_model_position_at_parameter(curve, &self.accepted_document, 0.5)
                {
                    anchors.push(DraftReferenceAnchor::Midpoint {
                        span: curve.span,
                        model_position,
                        affine_direction,
                        role: curve.role,
                        source_role: curve.source_role,
                        origin,
                    });
                }
            } else {
                nonlinear_samples.extend(samples.into_iter().map(|sample| (curve, origin, sample)));
            }
        }

        if let Err(evidence) = append_nonlinear_draft_anchors(
            &self.accepted_document,
            &mut nonlinear_samples,
            &mut anchors,
            limits.max_scene_anchors,
        ) {
            return DraftInferenceAnchorCollection::ResourceLimited(evidence);
        }
        DraftInferenceAnchorCollection::Complete { anchors }
    }

    /// Collects every exact scene input relevant to one inference subject.
    ///
    /// The ordinary M70 point/curve anchors and M71 semantic centers share one
    /// complete bound for centered tools. Circle-circumference inference only
    /// reads stored points, so unrelated curve tessellation cannot disable a
    /// valid through-point placement. Suppressed authoring bypasses this query
    /// entirely at the editor boundary.
    #[must_use]
    pub fn draft_inference_scene_inputs(
        &self,
        pointer: ScreenPoint,
        subject: DraftInferenceSubject,
        limits: DraftInferenceLimits,
    ) -> DraftInferenceSceneInputCollection {
        if !pointer.is_finite() {
            return DraftInferenceSceneInputCollection::Complete(DraftInferenceSceneInputs {
                anchors: Vec::new(),
                semantic_centers: Vec::new(),
            });
        }
        match subject {
            DraftInferenceSubject::PointOperand => {
                let anchors = match self.draft_inference_anchors(pointer, limits) {
                    DraftInferenceAnchorCollection::Complete { anchors } => anchors,
                    DraftInferenceAnchorCollection::ResourceLimited(evidence) => {
                        return DraftInferenceSceneInputCollection::ResourceLimited(evidence);
                    }
                };
                DraftInferenceSceneInputCollection::Complete(DraftInferenceSceneInputs {
                    anchors,
                    semantic_centers: Vec::new(),
                })
            }
            DraftInferenceSubject::CircleCircumference => {
                let required = self.construction_snap_points.len();
                if required > limits.max_scene_anchors {
                    return DraftInferenceSceneInputCollection::ResourceLimited(
                        DraftInferenceSceneLimit {
                            resource: DraftInferenceSceneResource::Anchors,
                            required,
                            limit: limits.max_scene_anchors,
                        },
                    );
                }
                let anchors = self
                    .construction_snap_points
                    .iter()
                    .map(|point| DraftReferenceAnchor::PersistentPoint {
                        point: point.id,
                        model_position: point.model_position,
                        role_incidence: point.role_incidence,
                    })
                    .collect();
                DraftInferenceSceneInputCollection::Complete(DraftInferenceSceneInputs {
                    anchors,
                    semantic_centers: Vec::new(),
                })
            }
            DraftInferenceSubject::CenteredPointOperand { .. } => {
                let semantic_centers = self.bounded_draft_semantic_center_anchors(limits);
                let semantic_centers = match semantic_centers {
                    Ok(centers) => centers,
                    Err(evidence) => {
                        return DraftInferenceSceneInputCollection::ResourceLimited(evidence);
                    }
                };
                let ordinary_limit = DraftInferenceLimits {
                    max_scene_anchors: limits
                        .max_scene_anchors
                        .saturating_sub(semantic_centers.len()),
                    ..limits
                };
                let anchors = match self.draft_inference_anchors(pointer, ordinary_limit) {
                    DraftInferenceAnchorCollection::Complete { anchors } => anchors,
                    DraftInferenceAnchorCollection::ResourceLimited(mut evidence) => {
                        if evidence.resource == DraftInferenceSceneResource::Anchors {
                            evidence.required =
                                evidence.required.saturating_add(semantic_centers.len());
                            evidence.limit = limits.max_scene_anchors;
                        }
                        return DraftInferenceSceneInputCollection::ResourceLimited(evidence);
                    }
                };
                DraftInferenceSceneInputCollection::Complete(DraftInferenceSceneInputs {
                    anchors,
                    semantic_centers,
                })
            }
        }
    }

    fn bounded_draft_semantic_center_anchors(
        &self,
        limits: DraftInferenceLimits,
    ) -> Result<Vec<DraftSemanticCenterAnchor>, DraftInferenceSceneLimit> {
        // Prove the existing scene-input bound before allocating the semantic
        // join set. Every eligible curve occurrence contributes at least one
        // ordinary anchor, so a successful allocation-free census also bounds
        // the number of distinct curve identities collected below.
        if let Some(evidence) = draft_inference_scene_resource_limit(self, limits) {
            return Err(evidence);
        }
        let eligible_curves = self
            .curves
            .iter()
            .filter(|curve| curve.authoring_eligible)
            .map(|curve| curve.span.curve)
            .collect::<std::collections::BTreeSet<_>>();
        let mut centers = Vec::new();
        for curve in eligible_curves {
            let Some(anchor) = (|| {
                let center = self
                    .accepted_document
                    .resolve_center_ref(DocumentCenterRef { curve })
                    .ok()?;
                let model_position = self.accepted_document.point(center)?.position;
                Some(DraftSemanticCenterAnchor {
                    curve,
                    center,
                    model_position,
                    role: self
                        .accepted_document
                        .geometry_role(curve)
                        .unwrap_or_default(),
                })
            })() else {
                continue;
            };
            centers.push(anchor);
            if centers.len() > limits.max_scene_anchors {
                return Err(DraftInferenceSceneLimit {
                    resource: DraftInferenceSceneResource::Anchors,
                    required: centers.len(),
                    limit: limits.max_scene_anchors,
                });
            }
        }
        Ok(centers)
    }

    /// Builds one composite scene from exact-stamped accepted sketch and computed
    /// output. Replaced native supports use evaluated source fragments, while
    /// generated arcs retain stable feature/corner selection provenance.
    ///
    /// The detached stamps validate computed-output provenance but do not grant
    /// inference-publication authority. Call [`Self::with_retained_session`] on
    /// the completed scene when that capability is required.
    ///
    /// # Errors
    ///
    /// Rejects stale/mismatched provenance or non-finite generated geometry.
    #[allow(
        clippy::too_many_arguments,
        clippy::too_many_lines,
        reason = "composite scene validation and native/computed publication remain one auditable boundary"
    )]
    pub fn from_accepted_with_computed(
        accepted_revision: u64,
        design_identity: SketchDesignIdentity,
        accepted_document: &SketchDocument,
        design_document: &SketchDocument,
        accepted_sketch_input: &PreparedSketchInput,
        expected: &geosolve_sketch_features::ComputedFeatureEvaluationInput,
        computed: &geosolve_sketch_features::ComputedFeatureSnapshot,
        viewport: Viewport,
        chord_tolerance_pixels: f64,
    ) -> Result<Self, EditorError> {
        let mut scene = Self::from_accepted_for_design(
            accepted_revision,
            design_identity,
            accepted_document,
            design_document,
            viewport,
            chord_tolerance_pixels,
        )?;
        if computed.input() != *expected
            || expected.sketch != *accepted_sketch_input
            || expected.sketch.accepted_state_identity() != Some(expected.accepted)
            || expected.accepted.revision().get() != accepted_revision
            || expected.accepted.document() != accepted_document.id()
            || expected.sketch.design_identity() != design_identity
            || expected.features.sketch_document != accepted_document.id()
        {
            return Err(EditorError::StaleComputedFeatureSnapshot);
        }
        let replaced = computed
            .replaced_sources()
            .iter()
            .map(|source| source.span)
            .collect::<std::collections::BTreeSet<_>>();
        scene.curves.retain(|curve| !replaced.contains(&curve.span));
        for edge in computed.edges() {
            match (&edge.geometry, &edge.provenance) {
                (
                    geosolve_sketch_features::ComputedEdgeGeometry::NativeSourceFragment {
                        source,
                        interval,
                    },
                    geosolve_sketch_features::ComputedEdgeProvenance::SourceFragment { .. },
                ) => {
                    let mut curve = scene_curve_for_interval(
                        accepted_document,
                        viewport,
                        source.span,
                        interval.start,
                        interval.end,
                        chord_tolerance_pixels,
                    )?;
                    curve.authoring_eligible = design_document
                        .curve_spans(curve.span.curve)
                        .is_ok_and(|spans| spans.contains(&curve.span));
                    curve.role = edge.role;
                    curve.source_role = accepted_document
                        .geometry_role(source.span.curve)
                        .unwrap_or_default();
                    scene.curves.push(curve);
                }
                (
                    geosolve_sketch_features::ComputedEdgeGeometry::CircularArc(arc),
                    geosolve_sketch_features::ComputedEdgeProvenance::FilletArc { owner, .. },
                ) => scene.computed_curves.push(SceneComputedCurve {
                    edge: edge.id,
                    owner: *owner,
                    role: edge.role,
                    center: arc.center,
                    radius: arc.radius,
                    start_angle: arc.start_angle,
                    end_angle: arc.end_angle,
                    sweep: arc.sweep,
                    contacts: arc.contacts,
                    screen_polyline: tessellate_computed_arc(
                        arc,
                        viewport,
                        chord_tolerance_pixels,
                    )?,
                    radius_rail: None,
                }),
                _ => {}
            }
        }
        for fragment in computed.construction_fragments() {
            let mut curve = scene_curve_for_interval(
                accepted_document,
                viewport,
                fragment.source.span,
                fragment.interval.start,
                fragment.interval.end,
                chord_tolerance_pixels,
            )?;
            curve.authoring_eligible = design_document
                .curve_spans(curve.span.curve)
                .is_ok_and(|spans| spans.contains(&curve.span));
            curve.role = GeometryRole::Construction;
            curve.source_role = fragment.source_role;
            curve.origin = SceneCurveOrigin::FilletDiscarded {
                fragment: fragment.id,
                source: fragment.source,
                interval: fragment.interval,
                provenance: fragment.provenance,
            };
            scene.curves.push(curve);
        }
        scene.curves.sort_by_key(|curve| curve.span);
        scene.computed_curves.sort_by_key(|curve| curve.edge);
        scene.feature_identity = Some(computed.input().features);
        scene.computed_input = Some(computed.input());
        // A detached input stamp cannot authenticate caller-supplied scene
        // geometry. Inference publication is enabled only by
        // `with_retained_session` after this composite scene is complete.
        scene.prepared_input = None;
        scene.fillet_affordances.clear();
        scene.annotations = annotations::build_annotations(
            accepted_document,
            &scene.points,
            &scene.curves,
            viewport,
        );
        scene.refresh_draft_inference_seal();
        Ok(scene)
    }

    /// Retains the exact pointer-down input while presenting a newer current
    /// computed-feature preview scene for the same sketch and feature sidecar.
    ///
    /// This stamp admits later samples only for the already-live gesture. It
    /// does not make the origin current or authorize a feature mutation.
    ///
    /// # Errors
    ///
    /// Rejects a scene without current computed output or an origin from a
    /// different sketch input, accepted state, feature document, or policy.
    pub fn set_computed_fillet_interaction_origin(
        &mut self,
        origin: geosolve_sketch_features::ComputedFeatureEvaluationInput,
    ) -> Result<(), EditorError> {
        let Some(current) = self.computed_input else {
            return Err(EditorError::StaleComputedFeatureSnapshot);
        };
        if self.feature_identity != Some(current.features)
            || current.sketch.design_identity() != self.design_identity
            || origin.sketch != current.sketch
            || origin.accepted != current.accepted
            || origin.policy != current.policy
            || origin.features.document != current.features.document
            || origin.features.sketch_document != current.features.sketch_document
        {
            return Err(EditorError::StaleComputedFeatureSnapshot);
        }
        self.fillet_interaction_origin = Some(origin);
        Ok(())
    }

    fn accepts_fillet_gesture(
        &self,
        expected: &geosolve_sketch_features::ComputedFeatureEvaluationInput,
    ) -> bool {
        self.computed_input.as_ref() == Some(expected)
            || self.fillet_interaction_origin.as_ref() == Some(expected)
    }

    /// Attaches one independently derived Fillet-radius continuation rail.
    ///
    /// The supplied derivative is model-space `d(center) / d(radius)` for the
    /// curve owner's current absolute branch. This method derives only finite
    /// display geometry; it does not resolve or select a Fillet root. The
    /// affected-owner list must be sorted, unique and contain the grip owner.
    ///
    /// # Errors
    ///
    /// Rejects a missing owner, malformed affected-owner list or a
    /// non-finite/degenerate derivative.
    pub fn attach_computed_fillet_radius_rail(
        &mut self,
        owner: geosolve_sketch_features::ComputedCornerRef,
        model_derivative: [f64; 2],
        affected_owners: Vec<geosolve_sketch_features::ComputedCornerRef>,
    ) -> Result<(), EditorError> {
        if affected_owners.binary_search(&owner).is_err()
            || affected_owners.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(EditorError::InvalidComputedFeatureAffordance);
        }
        let curve_index = self
            .computed_curves
            .iter()
            .position(|curve| curve.owner == owner)
            .ok_or(EditorError::StaleComputedFeatureSnapshot)?;
        let (rail, contacts) = Self::derive_computed_fillet_radius_affordances(
            &self.computed_curves[curve_index],
            self.viewport,
            owner,
            model_derivative,
        )?;
        self.computed_curves[curve_index].radius_rail = Some(rail);
        if let Some(affordances) = self
            .fillet_affordances
            .iter_mut()
            .find(|candidate| candidate.owner == owner)
        {
            affordances.affected_owners = affected_owners;
            affordances.radius_rail = rail;
            affordances.contacts = contacts;
        } else {
            self.fillet_affordances.push(SceneFilletCornerAffordances {
                owner,
                affected_owners,
                radius_rail: rail,
                contacts,
                actions: Vec::new(),
                continuation_status: None,
            });
            self.fillet_affordances
                .sort_by_key(|affordances| affordances.owner);
        }
        Ok(())
    }

    fn derive_computed_fillet_radius_affordances(
        curve: &SceneComputedCurve,
        viewport: Viewport,
        owner: geosolve_sketch_features::ComputedCornerRef,
        model_derivative: [f64; 2],
    ) -> Result<(SceneFilletRadiusRail, [SceneFilletContactHandle; 2]), EditorError> {
        const HALF_RAIL_PIXELS: f64 = 44.0;

        let tau = std::f64::consts::TAU;
        let delta = match curve.sweep {
            DocumentArcSweep::CounterClockwise => {
                (curve.end_angle - curve.start_angle).rem_euclid(tau)
            }
            DocumentArcSweep::Clockwise => -(curve.start_angle - curve.end_angle).rem_euclid(tau),
        };
        let middle_angle = (0.5 * delta).mul_add(1.0, curve.start_angle);
        let model_grip = [
            curve.radius.mul_add(middle_angle.cos(), curve.center[0]),
            curve.radius.mul_add(middle_angle.sin(), curve.center[1]),
        ];
        let screen_center = viewport.model_to_screen(curve.center);
        let screen_grip = viewport.model_to_screen(model_grip);
        let derivative_tip = viewport.model_to_screen([
            curve.center[0] + model_derivative[0],
            curve.center[1] + model_derivative[1],
        ]);
        let screen_derivative = [
            derivative_tip.x - screen_center.x,
            derivative_tip.y - screen_center.y,
        ];
        let screen_norm = screen_derivative[0].hypot(screen_derivative[1]);
        if !screen_norm.is_finite() || screen_norm <= 0.0 {
            return Err(EditorError::StaleComputedFeatureSnapshot);
        }
        let unit = [
            screen_derivative[0] / screen_norm,
            screen_derivative[1] / screen_norm,
        ];
        let rail = SceneFilletRadiusRail {
            owner,
            model_center: curve.center,
            model_grip,
            model_derivative,
            screen_center,
            screen_grip,
            screen_rail_start: ScreenPoint {
                x: (-HALF_RAIL_PIXELS).mul_add(unit[0], screen_grip.x),
                y: (-HALF_RAIL_PIXELS).mul_add(unit[1], screen_grip.y),
            },
            screen_rail_end: ScreenPoint {
                x: HALF_RAIL_PIXELS.mul_add(unit[0], screen_grip.x),
                y: HALF_RAIL_PIXELS.mul_add(unit[1], screen_grip.y),
            },
        };
        if !rail.is_valid() {
            return Err(EditorError::StaleComputedFeatureSnapshot);
        }
        let contacts = [
            SceneFilletContactHandle {
                owner,
                parent: ComputedFilletParentIndex::First,
                source: curve.contacts[0].source,
                parameter: curve.contacts[0].parameter,
                model_position: curve.contacts[0].position,
                screen_position: viewport.model_to_screen(curve.contacts[0].position),
            },
            SceneFilletContactHandle {
                owner,
                parent: ComputedFilletParentIndex::Second,
                source: curve.contacts[1].source,
                parameter: curve.contacts[1].parameter,
                model_position: curve.contacts[1].position,
                screen_position: viewport.model_to_screen(curve.contacts[1].position),
            },
        ];
        if !contacts
            .iter()
            .copied()
            .all(SceneFilletContactHandle::is_valid)
        {
            return Err(EditorError::InvalidComputedFeatureAffordance);
        }
        Ok((rail, contacts))
    }

    /// Replaces coordinator-supplied actions for one current Fillet corner.
    ///
    /// This validates stable owner identity, unique action IDs, non-empty
    /// accessibility text and finite optional preview polylines. It does not
    /// reinterpret applicability or generate branch alternatives.
    ///
    /// # Errors
    ///
    /// Rejects a missing owner or malformed presentation data.
    pub fn set_fillet_corner_actions(
        &mut self,
        owner: geosolve_sketch_features::ComputedCornerRef,
        actions: Vec<SceneFilletAction>,
    ) -> Result<(), EditorError> {
        let affordances = self
            .fillet_affordances
            .iter_mut()
            .find(|candidate| candidate.owner == owner)
            .ok_or(EditorError::InvalidComputedFeatureAffordance)?;
        let mut ids = Vec::with_capacity(actions.len());
        if actions.iter().any(|action| {
            let duplicate = ids.contains(&action.id);
            ids.push(action.id);
            !action.is_valid(owner, self.viewport) || duplicate
        }) {
            return Err(EditorError::InvalidComputedFeatureAffordance);
        }
        affordances.actions = actions;
        Ok(())
    }

    /// Attaches the active gesture's typed continuation limit to one exact corner.
    ///
    /// The status may refer to the retained pointer-down input while the scene
    /// presents a newer last-current preview, but owner and named contact source
    /// must still match this scene's independently derived affordances.
    ///
    /// # Errors
    ///
    /// Rejects a missing/foreign owner, stale input, malformed message/sample or
    /// a contact status attributed to the wrong named parent.
    pub fn set_computed_fillet_continuation_status(
        &mut self,
        owner: geosolve_sketch_features::ComputedCornerRef,
        status: Option<ComputedFilletContinuationStatus>,
    ) -> Result<(), EditorError> {
        let curve_index = self
            .computed_curves
            .iter()
            .position(|candidate| candidate.owner == owner)
            .ok_or(EditorError::InvalidComputedFeatureAffordance)?;
        if let Some(status) = status.as_ref() {
            if status.owner != owner
                || !status.is_valid()
                || !self.accepts_fillet_gesture(&status.expected)
            {
                return Err(EditorError::InvalidComputedFeatureAffordance);
            }
            if let ComputedFilletInteractionSample::Contact { parent, source, .. } = status.sample {
                let expected_contact = match parent {
                    ComputedFilletParentIndex::First => {
                        self.computed_curves[curve_index].contacts[0]
                    }
                    ComputedFilletParentIndex::Second => {
                        self.computed_curves[curve_index].contacts[1]
                    }
                };
                if expected_contact.source != source {
                    return Err(EditorError::InvalidComputedFeatureAffordance);
                }
            }
        }
        self.computed_fillet_continuation_statuses
            .retain(|candidate| candidate.owner != owner);
        if let Some(status) = status.clone() {
            self.computed_fillet_continuation_statuses.push(status);
            self.computed_fillet_continuation_statuses
                .sort_by_key(|candidate| candidate.owner);
        }
        if let Some(affordances) = self
            .fillet_affordances
            .iter_mut()
            .find(|candidate| candidate.owner == owner)
        {
            affordances.continuation_status = status;
        }
        Ok(())
    }

    /// Tessellates one independently validated alternative Fillet arc for presentation.
    ///
    /// The caller supplies the semantic alternative and pixel tolerance; this
    /// method applies the same bounded computed-arc sampling used by the current
    /// scene without resolving or accepting a branch.
    ///
    /// # Errors
    ///
    /// Rejects non-finite geometry or a non-positive/non-finite tolerance.
    pub fn tessellate_computed_fillet_arc(
        &self,
        arc: &ComputedCircularArc,
        chord_tolerance_pixels: f64,
    ) -> Result<SceneFilletAlternativeGeometry, EditorError> {
        if !chord_tolerance_pixels.is_finite() || chord_tolerance_pixels <= 0.0 {
            return Err(EditorError::InvalidTolerance);
        }
        tessellate_computed_arc_geometry(arc, self.viewport, chord_tolerance_pixels)
    }

    /// Returns the exact scene-stamped target for one advertised Fillet action.
    ///
    /// Disabled actions still have targets so presentation can retain stable
    /// focus metadata; the shared resolver rejects them for preview/activation.
    #[must_use]
    pub fn fillet_action_target(
        &self,
        owner: geosolve_sketch_features::ComputedCornerRef,
        action: SceneFilletActionId,
    ) -> Option<SceneFilletActionTarget> {
        let expected = self.computed_input?;
        self.fillet_affordances
            .iter()
            .find(|affordances| affordances.owner == owner)?
            .actions
            .iter()
            .any(|candidate| candidate.owner == owner && candidate.id == action)
            .then_some(SceneFilletActionTarget {
                expected,
                owner,
                action,
            })
    }

    /// Resolves canvas and accessible Fillet actions through one exact validator.
    ///
    /// Canvas painted identity is a non-authoritative hint. A painted branch
    /// control that independently matches its paired model-space geometry wins
    /// over an overlapping radius surface; otherwise the ordinary Fillet hit
    /// resolver remains authoritative. An accessible target skips pointer
    /// proximity but retains identical scene, owner, action and applicability
    /// checks.
    #[must_use]
    pub fn resolve_fillet_action(
        &self,
        input: SceneFilletActionInput,
        tolerance: PickTolerance,
    ) -> Option<SceneFilletActionTarget> {
        self.resolve_fillet_action_with_policy(
            input,
            tolerance,
            GeometryInteractionPolicy::default(),
        )
    }

    /// Policy-aware counterpart of [`Self::resolve_fillet_action`]. Both canvas
    /// and accessible inputs require the owning computed curve to be visible
    /// and admitted by the current pick scope.
    #[must_use]
    pub fn resolve_fillet_action_with_policy(
        &self,
        input: SceneFilletActionInput,
        tolerance: PickTolerance,
        policy: GeometryInteractionPolicy,
    ) -> Option<SceneFilletActionTarget> {
        if !tolerance.is_valid() {
            return None;
        }
        match input {
            SceneFilletActionInput::Accessible(target) => self
                .computed_owner_is_interactive(target.owner, policy)
                .then(|| self.validated_fillet_action(&target))
                .flatten()
                .map(|_| target),
            SceneFilletActionInput::Canvas { position, painted } => {
                if !position.is_finite() {
                    return None;
                }
                let expected = self.computed_input?;
                let model_position = self.viewport.screen_to_model(position);
                let (resolved, _) = self
                    .fillet_affordances
                    .iter()
                    .flat_map(|affordances| &affordances.actions)
                    .filter(|action| self.computed_owner_is_interactive(action.owner, policy))
                    .filter(|action| {
                        matches!(
                            action.availability,
                            SceneFilletActionAvailability::Applicable
                        )
                    })
                    .filter_map(|action| {
                        let target = SceneFilletActionTarget {
                            expected,
                            owner: action.owner,
                            action: action.id,
                        };
                        action_model_hit_distance(action, model_position, self.viewport, tolerance)
                            .map(|distance| (target, distance))
                    })
                    .min_by(|first, second| {
                        first
                            .1
                            .total_cmp(&second.1)
                            .then_with(|| first.0.owner.cmp(&second.0.owner))
                            .then_with(|| {
                                fillet_action_order_key(first.0.action)
                                    .cmp(&fillet_action_order_key(second.0.action))
                            })
                    })?;
                if painted.is_some_and(|hint| hint != resolved) {
                    return None;
                }
                self.validated_fillet_action(&resolved).map(|_| resolved)
            }
        }
    }

    fn computed_owner_is_interactive(
        &self,
        owner: geosolve_sketch_features::ComputedCornerRef,
        policy: GeometryInteractionPolicy,
    ) -> bool {
        self.computed_curves
            .iter()
            .find(|curve| curve.owner == owner)
            .is_some_and(|curve| curve.is_interactive(policy))
    }

    fn validated_fillet_action(
        &self,
        target: &SceneFilletActionTarget,
    ) -> Option<&SceneFilletAction> {
        if self.computed_input != Some(target.expected)
            || self.feature_identity != Some(target.expected.features)
        {
            return None;
        }
        self.fillet_affordances
            .iter()
            .find(|affordances| affordances.owner == target.owner)?
            .actions
            .iter()
            .find(|action| {
                action.owner == target.owner
                    && action.id == target.action
                    && matches!(
                        action.availability,
                        SceneFilletActionAvailability::Applicable
                    )
                    && action.is_valid(target.owner, self.viewport)
            })
    }

    /// Resolves one Fillet-aware hit through the shared headless priority.
    ///
    /// The visible radius grip/spoke/rail/arc wins over native accepted points
    /// and curves. Endpoint contact metadata is deliberately not a canvas hit
    /// target. Constraint and dimension annotations remain a separate
    /// presentation layer.
    #[must_use]
    pub fn resolve_fillet_hit(
        &self,
        position: ScreenPoint,
        tolerance: PickTolerance,
    ) -> Option<SceneFilletHit> {
        self.resolve_fillet_hit_with_policy(
            position,
            tolerance,
            GeometryInteractionPolicy::default(),
        )
    }

    /// Fillet-aware counterpart of [`Self::hit_test_with_policy`].
    #[must_use]
    pub fn resolve_fillet_hit_with_policy(
        &self,
        position: ScreenPoint,
        tolerance: PickTolerance,
        policy: GeometryInteractionPolicy,
    ) -> Option<SceneFilletHit> {
        if !position.is_finite() || !tolerance.is_valid() {
            return None;
        }
        if let Some((owner, distance_pixels)) = self
            .fillet_affordances
            .iter()
            .filter(|affordances| {
                self.computed_curves
                    .iter()
                    .find(|curve| curve.owner == affordances.owner)
                    .is_some_and(|curve| curve.is_pickable(policy))
            })
            .filter_map(|affordances| {
                self.fillet_radius_hit_distance(affordances, position, tolerance)
                    .map(|distance| (affordances.owner, distance))
            })
            .min_by(|first, second| {
                first
                    .1
                    .total_cmp(&second.1)
                    .then_with(|| first.0.cmp(&second.0))
            })
        {
            return Some(SceneFilletHit::Radius {
                owner,
                distance_pixels,
            });
        }
        self.native_authoring_hit_test_with_policy(position, tolerance, policy)
            .map(SceneFilletHit::Native)
    }

    fn fillet_radius_hit_distance(
        &self,
        affordances: &SceneFilletCornerAffordances,
        position: ScreenPoint,
        tolerance: PickTolerance,
    ) -> Option<f64> {
        let rail = affordances.radius_rail;
        let grip = position.distance(rail.screen_grip);
        let spoke = point_segment_projection(position, rail.screen_center, rail.screen_grip).0;
        let continuation =
            point_segment_projection(position, rail.screen_rail_start, rail.screen_rail_end).0;
        let arc = self
            .computed_curves
            .iter()
            .find(|curve| curve.owner == affordances.owner)
            .and_then(|curve| computed_curve_hit(curve, position, tolerance.curve_pixels))
            .map(|hit| hit.distance_pixels);
        [
            (grip <= tolerance.point_pixels).then_some(grip),
            (spoke <= tolerance.curve_pixels).then_some(spoke),
            (continuation <= tolerance.curve_pixels).then_some(continuation),
            arc,
        ]
        .into_iter()
        .flatten()
        .min_by(f64::total_cmp)
    }

    fn project_to_native_source(
        &self,
        source: NativeCurveSpanSource,
        position: ScreenPoint,
    ) -> Option<(f64, f64)> {
        self.curves
            .iter()
            .filter(|curve| curve.span == source.span)
            .filter_map(|curve| curve_hit(curve, position, f64::MAX))
            .filter_map(|hit| {
                hit.curve_parameter
                    .map(|parameter| (parameter, hit.distance_pixels))
            })
            .min_by(|first, second| first.1.total_cmp(&second.1))
    }

    /// Returns the deterministic best hit. Points take priority over curves, then
    /// distance and persistent identity break ties.
    #[must_use]
    pub fn hit_test(&self, position: ScreenPoint, tolerance: PickTolerance) -> Option<Hit> {
        self.hit_test_with_policy(position, tolerance, GeometryInteractionPolicy::default())
    }

    /// Returns the deterministic best visible hit under one complete headless
    /// geometry policy.
    ///
    /// In `All`, a Profile candidate wins a cross-role near-tie of at most one
    /// CSS pixel; cross-role candidates outside that band use distance. Within
    /// one role, semantic point/curve kind precedes distance. Persistent
    /// identity and picked parameter finish deterministic ties.
    #[must_use]
    pub fn hit_test_with_policy(
        &self,
        position: ScreenPoint,
        tolerance: PickTolerance,
        policy: GeometryInteractionPolicy,
    ) -> Option<Hit> {
        if !position.is_finite() || !tolerance.is_valid() {
            return None;
        }
        self.geometry_hit_test_with_policy(position, tolerance, policy)
            .or_else(|| self.datum_hit_test(position, policy))
    }

    fn geometry_hit_test_with_policy(
        &self,
        position: ScreenPoint,
        tolerance: PickTolerance,
        policy: GeometryInteractionPolicy,
    ) -> Option<Hit> {
        best_policy_hit(
            self.geometry_hit_test_candidates(position, tolerance, policy),
            policy.scope,
        )
    }

    fn draggable_geometry_hit_test_with_policy(
        &self,
        position: ScreenPoint,
        tolerance: PickTolerance,
        policy: GeometryInteractionPolicy,
    ) -> Option<Hit> {
        let hits = self
            .geometry_hit_test_candidates(position, tolerance, policy)
            .filter(|hit| {
                self.drag_handle_point(hit.item).is_some()
                    || self.feature_radius_handle(hit.item).is_some()
            });
        best_policy_hit(hits, policy.scope)
    }

    fn geometry_hit_test_candidates(
        &self,
        position: ScreenPoint,
        tolerance: PickTolerance,
        policy: GeometryInteractionPolicy,
    ) -> impl Iterator<Item = Hit> + '_ {
        self.points
            .iter()
            .filter(move |point| point.is_pickable(policy))
            .filter_map(move |point| point_hit(point, position, tolerance.point_pixels))
            .chain(
                self.curves
                    .iter()
                    .filter(move |curve| curve.is_pickable(policy))
                    .filter_map(move |curve| curve_hit(curve, position, tolerance.curve_pixels)),
            )
            .chain(
                self.computed_curves
                    .iter()
                    .filter(move |curve| curve.is_pickable(policy))
                    .filter_map(move |curve| {
                        computed_curve_hit(curve, position, tolerance.curve_pixels)
                    }),
            )
    }

    fn datum_hit_test(
        &self,
        position: ScreenPoint,
        policy: GeometryInteractionPolicy,
    ) -> Option<Hit> {
        if !policy.visibility.reference_geometry {
            return None;
        }
        if let Some(origin) = self.datums.iter().find(|datum| {
            datum.datum == SketchDatum::Origin && datum.is_visible_in_viewport(self.viewport)
        }) {
            let origin_distance = position.distance(origin.screen_start);
            if origin_distance <= 6.0 {
                return Some(Hit {
                    item: SelectionItem::Datum(SketchDatum::Origin),
                    distance_pixels: origin_distance,
                    curve_parameter: None,
                    geometry: None,
                });
            }
        }
        self.datums
            .iter()
            .filter(|datum| datum.datum != SketchDatum::Origin)
            .filter(|datum| datum.is_visible_in_viewport(self.viewport))
            .filter_map(|datum| {
                let distance =
                    point_segment_projection(position, datum.screen_start, datum.screen_end).0;
                (distance <= 4.0).then_some(Hit {
                    item: SelectionItem::Datum(datum.datum),
                    distance_pixels: distance,
                    curve_parameter: None,
                    geometry: None,
                })
            })
            .min_by(|first, second| {
                first
                    .distance_pixels
                    .total_cmp(&second.distance_pixels)
                    .then_with(|| first.item.cmp(&second.item))
            })
    }

    /// Native-only geometry hit for constraint and computed-feature authoring.
    /// Generated arcs are deliberately ignored rather than becoming operands or
    /// blocking their trimmed source fragments underneath.
    #[must_use]
    pub fn native_authoring_hit_test(
        &self,
        position: ScreenPoint,
        tolerance: PickTolerance,
    ) -> Option<Hit> {
        self.native_authoring_hit_test_with_policy(
            position,
            tolerance,
            GeometryInteractionPolicy::default(),
        )
    }

    /// Native-only authoring hit under one complete geometry policy.
    #[must_use]
    pub fn native_authoring_hit_test_with_policy(
        &self,
        position: ScreenPoint,
        tolerance: PickTolerance,
        policy: GeometryInteractionPolicy,
    ) -> Option<Hit> {
        if !position.is_finite() || !tolerance.is_valid() {
            return None;
        }
        best_policy_hit(
            self.points
                .iter()
                .filter(|point| point.is_pickable(policy))
                .filter_map(|point| point_hit(point, position, tolerance.point_pixels))
                .chain(
                    self.curves
                        .iter()
                        .filter(|curve| curve.is_pickable(policy))
                        .filter_map(|curve| curve_hit(curve, position, tolerance.curve_pixels)),
                ),
            policy.scope,
        )
    }

    /// Returns bounded native and intrinsic-datum authoring hits in deterministic
    /// interaction order.
    ///
    /// Point candidates retain their semantic priority over curves, while the
    /// complete ordered list lets a domain-specific headless authoring state
    /// reject an inapplicable point or already-pending support and continue to
    /// another curve under the same click. Repeated visible fragments of one
    /// persistent span contribute only their nearest occurrence and count once
    /// against `maximum_candidates`. Collection stops before allocating a
    /// candidate beyond that explicit limit.
    ///
    /// # Errors
    ///
    /// Returns [`NativeAuthoringHitError::CandidateLimitExceeded`] on the first
    /// distinct in-tolerance item beyond `maximum_candidates`.
    #[cfg(test)]
    pub(crate) fn native_authoring_hit_candidates(
        &self,
        position: ScreenPoint,
        tolerance: PickTolerance,
        maximum_candidates: usize,
    ) -> Result<Vec<Hit>, NativeAuthoringHitError> {
        self.native_authoring_hit_candidates_with_policy(
            position,
            tolerance,
            maximum_candidates,
            GeometryInteractionPolicy::default(),
        )
    }

    pub(crate) fn native_authoring_hit_candidates_with_policy(
        &self,
        position: ScreenPoint,
        tolerance: PickTolerance,
        maximum_candidates: usize,
        policy: GeometryInteractionPolicy,
    ) -> Result<Vec<Hit>, NativeAuthoringHitError> {
        if !position.is_finite() || !tolerance.is_valid() {
            return Ok(Vec::new());
        }
        let native_candidates = self
            .points
            .iter()
            .filter(|point| point.is_pickable(policy))
            .filter_map(|point| point_hit(point, position, tolerance.point_pixels))
            .chain(
                self.curves
                    .iter()
                    .filter(|curve| curve.is_pickable(policy))
                    .filter_map(|curve| curve_hit(curve, position, tolerance.curve_pixels)),
            );
        let mut unique = std::collections::BTreeMap::<SelectionItem, PolicyHitAccumulator>::new();
        for hit in native_candidates {
            if let Some(existing) = unique.get_mut(&hit.item) {
                existing.consider(hit, policy.scope);
                continue;
            }
            if unique.len() >= maximum_candidates {
                return Err(NativeAuthoringHitError::CandidateLimitExceeded { maximum_candidates });
            }
            let mut accumulator = PolicyHitAccumulator::default();
            accumulator.consider(hit, policy.scope);
            unique.insert(hit.item, accumulator);
        }
        let mut remaining = unique
            .into_values()
            .filter_map(|candidate| candidate.best(policy.scope))
            .collect::<Vec<_>>();
        let mut ordered = Vec::with_capacity(remaining.len());
        while let Some(best) = best_policy_hit(remaining.iter().copied(), policy.scope) {
            let index = remaining
                .iter()
                .position(|candidate| *candidate == best)
                .expect("the selected authoring hit came from the remaining set");
            ordered.push(remaining.remove(index));
        }
        if let Some(hit) = self.datum_hit_test(position, policy) {
            if ordered.len() >= maximum_candidates {
                return Err(NativeAuthoringHitError::CandidateLimitExceeded { maximum_candidates });
            }
            ordered.push(hit);
        }
        Ok(ordered)
    }

    /// Returns the ordinary best visible geometry hit only when that exact
    /// persistent item still exists in `source`.
    ///
    /// This is the operation-authoring boundary for scenes containing accepted
    /// preview geometry. A preview-created foreground item, including one tied
    /// exactly with an older source item, blocks the pick; the search never
    /// clicks through it to source geometry underneath.
    #[must_use]
    pub fn hit_test_for_document(
        &self,
        position: ScreenPoint,
        tolerance: PickTolerance,
        source: &SketchDocument,
    ) -> Option<Hit> {
        self.hit_test_for_document_with_policy(
            position,
            tolerance,
            source,
            GeometryInteractionPolicy::default(),
        )
    }

    /// Operation-authoring hit under one complete geometry policy.
    #[must_use]
    pub fn hit_test_for_document_with_policy(
        &self,
        position: ScreenPoint,
        tolerance: PickTolerance,
        source: &SketchDocument,
        policy: GeometryInteractionPolicy,
    ) -> Option<Hit> {
        let hit = self.hit_test_with_policy(position, tolerance, policy)?;
        if !document_contains_item(source, hit.item) {
            return None;
        }
        let foreground_blocks = match hit.item {
            SelectionItem::Point(_) => self
                .points
                .iter()
                .filter(|point| point.is_pickable(policy))
                .any(|point| {
                    !document_contains_item(source, SelectionItem::Point(point.id))
                        && point_hit(point, position, tolerance.point_pixels).is_some_and(
                            |candidate| candidate.distance_pixels <= hit.distance_pixels,
                        )
                }),
            SelectionItem::Curve(_) => self
                .curves
                .iter()
                .filter(|curve| curve.is_pickable(policy))
                .any(|curve| {
                    !document_contains_item(source, SelectionItem::Curve(curve.span))
                        && curve_hit(curve, position, tolerance.curve_pixels).is_some_and(
                            |candidate| candidate.distance_pixels <= hit.distance_pixels,
                        )
                }),
            SelectionItem::Constraint(_)
            | SelectionItem::Dimension(_)
            | SelectionItem::Datum(_)
            | SelectionItem::Feature(_)
            | SelectionItem::FeatureCorner(_) => true,
        };
        (!foreground_blocks).then_some(hit)
    }

    fn drag_handle_point(&self, item: SelectionItem) -> Option<DesignPointId> {
        match item {
            SelectionItem::Point(point) => Some(point),
            SelectionItem::Curve(span) => self
                .curves
                .iter()
                .find(|curve| curve.span == span)
                .and_then(|curve| curve.drag_handle_point),
            SelectionItem::Constraint(_)
            | SelectionItem::Dimension(_)
            | SelectionItem::Datum(_)
            | SelectionItem::Feature(_)
            | SelectionItem::FeatureCorner(_) => None,
        }
    }

    fn feature_radius_handle(
        &self,
        item: SelectionItem,
    ) -> Option<(
        geosolve_sketch_features::ComputedCornerRef,
        f64,
        SceneFilletRadiusRail,
    )> {
        let SelectionItem::FeatureCorner(owner) = item else {
            return None;
        };
        self.computed_curves
            .iter()
            .find(|curve| curve.owner == owner)
            .and_then(|curve| curve.radius_rail.map(|rail| (owner, curve.radius, rail)))
    }

    /// Composite model bounds used by camera-fit policy.
    #[must_use]
    pub fn model_bounds(&self) -> Option<([f64; 2], [f64; 2])> {
        let mut points = self
            .points
            .iter()
            .map(|point| point.model_position)
            .chain(self.curves.iter().flat_map(|curve| {
                curve
                    .screen_polyline
                    .iter()
                    .map(|point| self.viewport.screen_to_model(*point))
            }))
            .chain(self.computed_curves.iter().flat_map(|curve| {
                curve
                    .screen_polyline
                    .iter()
                    .map(|point| self.viewport.screen_to_model(*point))
            }));
        let first = points.next()?;
        Some(
            points.fold((first, first), |(mut lower, mut upper), point| {
                lower[0] = lower[0].min(point[0]);
                lower[1] = lower[1].min(point[1]);
                upper[0] = upper[0].max(point[0]);
                upper[1] = upper[1].max(point[1]);
                (lower, upper)
            }),
        )
    }

    /// Returns the nearest visible annotation hit, preserving persistent identity.
    #[must_use]
    pub fn annotation_hit_test(
        &self,
        position: ScreenPoint,
        tolerance: PickTolerance,
        selection: &[SelectionItem],
        visibility_context: Option<SelectionItem>,
        problem_items: &[SelectionItem],
    ) -> Option<Hit> {
        self.annotation_occurrence_hit_test(
            position,
            tolerance,
            selection,
            visibility_context,
            problem_items,
        )
        .map(|(occurrence, distance_pixels)| Hit {
            item: occurrence.item,
            distance_pixels,
            curve_parameter: None,
            geometry: None,
        })
    }

    fn annotation_occurrence_hit_test(
        &self,
        position: ScreenPoint,
        tolerance: PickTolerance,
        selection: &[SelectionItem],
        visibility_context: Option<SelectionItem>,
        problem_items: &[SelectionItem],
    ) -> Option<(SceneAnnotationOccurrence, f64)> {
        if !position.is_finite() || !tolerance.is_valid() {
            return None;
        }
        self.annotations
            .iter()
            .filter(|annotation| {
                (self.show_all_constraint_annotations
                    && matches!(annotation.kind, SceneAnnotationKind::Constraint(_)))
                    || annotation.is_visible(selection, visibility_context, problem_items)
            })
            .filter_map(|annotation| {
                let (marker_index, distance) =
                    annotation.proximity_hit(position, tolerance.annotation_pixels)?;
                Some((
                    SceneAnnotationOccurrence {
                        item: annotation.item,
                        marker_index,
                    },
                    distance,
                ))
            })
            .min_by(|first, second| {
                first
                    .1
                    .total_cmp(&second.1)
                    .then_with(|| first.0.item.cmp(&second.0.item))
                    .then_with(|| first.0.marker_index.cmp(&second.0.marker_index))
            })
    }

    fn contextual_annotation_transit(
        &self,
        position: ScreenPoint,
        tolerance: PickTolerance,
        selection: &[SelectionItem],
        context_owner: SelectionItem,
        context_origin: ScreenPoint,
        problem_items: &[SelectionItem],
    ) -> bool {
        if !position.is_finite() || !context_origin.is_finite() || !tolerance.is_valid() {
            return false;
        }
        let corridor_tolerance = tolerance.annotation_pixels.max(14.0);
        let related = self
            .annotations
            .iter()
            .filter(|annotation| {
                annotation.item == context_owner || annotation.operands.contains(&context_owner)
            })
            .filter(|annotation| {
                (self.show_all_constraint_annotations
                    && matches!(annotation.kind, SceneAnnotationKind::Constraint(_)))
                    || annotation.is_visible(selection, Some(context_owner), problem_items)
            })
            .collect::<Vec<_>>();
        if related.iter().any(|annotation| {
            annotation.context_hit_test(position, context_origin, corridor_tolerance)
        }) {
            return true;
        }
        let anchors = related
            .iter()
            .flat_map(|annotation| annotation.context_anchors())
            .collect::<Vec<_>>();
        anchors.iter().enumerate().any(|(first_index, first)| {
            anchors.iter().skip(first_index + 1).any(|second| {
                point_segment_projection(position, *first, *second).0 <= corridor_tolerance
            })
        })
    }
}

/// Screen-space picking tolerances. These are interaction policy, not geometry tolerance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PickTolerance {
    pub point_pixels: f64,
    pub curve_pixels: f64,
    pub annotation_pixels: f64,
}

impl Default for PickTolerance {
    fn default() -> Self {
        Self {
            point_pixels: 8.0,
            curve_pixels: 12.0,
            annotation_pixels: 10.0,
        }
    }
}

impl PickTolerance {
    fn is_valid(self) -> bool {
        self.point_pixels.is_finite()
            && self.point_pixels >= 0.0
            && self.curve_pixels.is_finite()
            && self.curve_pixels >= 0.0
            && self.annotation_pixels.is_finite()
            && self.annotation_pixels >= 0.0
    }
}

/// Result of a deterministic scene hit test.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SceneGeometryHit {
    Point {
        incidence: ScenePointRoleIncidence,
    },
    NativeCurve {
        role: GeometryRole,
        source_role: GeometryRole,
        origin: SceneCurveOrigin,
    },
    ComputedFilletArc {
        edge: ComputedEdgeId,
        owner: ComputedCornerRef,
        role: GeometryRole,
    },
}

impl SceneGeometryHit {
    fn preferred_role(self, scope: GeometryPickScope) -> GeometryRole {
        match self {
            Self::Point { incidence } => incidence.preferred_role(scope),
            Self::NativeCurve { role, .. } | Self::ComputedFilletArc { role, .. } => role,
        }
    }
}

/// Result of a deterministic scene hit test.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub item: SelectionItem,
    pub distance_pixels: f64,
    /// Explicit curve feature picked by the user, when the hit is a curve.
    pub curve_parameter: Option<f64>,
    /// Exact visible occurrence responsible for this hit.
    ///
    /// Discarded Fillet fragments retain their evaluation-local provenance here
    /// while `item` remains the complete native source span.
    pub geometry: Option<SceneGeometryHit>,
}

/// Typed bounded-work result for native authoring hit collection.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum NativeAuthoringHitError {
    #[error("native authoring hit candidate limit {maximum_candidates} was exceeded")]
    CandidateLimitExceeded { maximum_candidates: usize },
}

/// One concrete presentation occurrence of a persistent scene annotation.
///
/// Multi-operand constraints can render more than one glyph while still mapping
/// every occurrence back to the same persistent constraint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SceneAnnotationOccurrence {
    pub item: SelectionItem,
    /// The deterministic marker position for glyph annotations. Dimensions have
    /// one presentation occurrence and therefore use `None`.
    pub marker_index: Option<usize>,
}

/// Exact pointer-proximity target owned by the headless editor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorHoverTarget {
    Geometry(SelectionItem),
    /// One transient selected-curve grip. `owner` remains the persistent
    /// selection; `control` identifies the exact visible affordance.
    CurveControl {
        control: DocumentCurveControlId,
        owner: CurveSpan,
    },
    Annotation(SceneAnnotationOccurrence),
}

impl EditorHoverTarget {
    /// Returns the persistent item represented by this proximity target.
    #[must_use]
    pub const fn item(self) -> SelectionItem {
        match self {
            Self::Geometry(item) => item,
            Self::CurveControl { owner, .. } => SelectionItem::Curve(owner),
            Self::Annotation(occurrence) => occurrence.item,
        }
    }
}

/// Presentation-neutral hover state separating proximity from revealed context.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EditorHoverState {
    /// Exact geometry or annotation occurrence currently under the pointer.
    pub target: Option<EditorHoverTarget>,
    /// Geometry whose directly related contextual annotations remain revealed.
    ///
    /// This remains stable while the pointer crosses a bounded navigation
    /// corridor or a related annotation occurrence.
    pub context_owner: Option<SelectionItem>,
}

/// Platform-independent modifier state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub command: bool,
}

impl Modifiers {
    #[must_use]
    pub const fn extends_selection(self) -> bool {
        self.shift || self.control || self.command
    }
}

/// Normalized pointer sample supplied by any presentation toolkit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerInput {
    pub pointer_id: u64,
    pub position: ScreenPoint,
    pub modifiers: Modifiers,
}

/// Kind of presentation pointer currently owned by the headless editor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivePointerGestureKind {
    Point,
    CurveControl,
    Annotation,
    FilletRadius,
    FilletContact,
}

/// Minimal pointer-capture signal for a presentation adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivePointerGesture {
    pub pointer_id: u64,
    pub kind: ActivePointerGestureKind,
}

/// Host work requested by one state transition.
#[derive(Clone, Debug, PartialEq)]
pub enum EditorEffect {
    SelectionChanged(Vec<SelectionItem>),
    /// Exact pointer proximity or its retained geometry context changed.
    HoverChanged(EditorHoverState),
    PreviewPointMove {
        point: DesignPointId,
        model_position: [f64; 2],
    },
    /// Requests a host solve/projection for one transient point-drag target.
    RequestProjectedPointMove {
        pointer_id: u64,
        request_id: u64,
        point: DesignPointId,
        model_position: [f64; 2],
    },
    /// Commits the retained projected point preview as one transaction. The
    /// retained coordinator clears transient point state only after this
    /// effect succeeds; on failure it keeps the last complete preview so the
    /// host can report the limit or retry without dispatching a separate
    /// [`Self::ClearPointPreview`].
    CommitPointMove {
        expected: SketchDesignIdentity,
        point: DesignPointId,
        model_position: [f64; 2],
    },
    ClearPointPreview,
    /// Requests one accepted-domain inverse curve-control preview from the
    /// exact scene/design captured at pointer-down.
    RequestCurveControlPreview {
        pointer_id: u64,
        request_id: u64,
        expected: SketchDesignIdentity,
        control: DocumentCurveControlId,
        model_position: [f64; 2],
    },
    /// Confirms that the exact request produced a finite independently accepted
    /// prepared candidate. Hosts render the candidate scene, while this signal
    /// preserves presentation-adapter parity with point previews.
    PreviewCurveControl {
        control: DocumentCurveControlId,
        model_position: [f64; 2],
    },
    /// Publishes the exact last accepted prepared request. The host must commit
    /// the retained candidate rather than reapplying `model_position`.
    CommitCurveControl {
        expected: SketchDesignIdentity,
        pointer_id: u64,
        request_id: u64,
        control: DocumentCurveControlId,
    },
    /// Clears any non-authoritative prepared curve-control preview.
    ClearCurveControlPreview,
    /// Shared-radius preview for one computed Fillet set. Native sketch points
    /// and equations are never part of this gesture.
    PreviewComputedFeatureRadius {
        expected: geosolve_sketch_features::ComputedFeatureEvaluationInput,
        feature: geosolve_sketch_features::ComputedFeatureId,
        radius: f64,
    },
    CommitComputedFeatureRadius {
        expected: geosolve_sketch_features::ComputedFeatureEvaluationInput,
        feature: geosolve_sketch_features::ComputedFeatureId,
        radius: f64,
    },
    /// Restores the exact pointer-down radius after a cancelled gesture. A
    /// grouped-authoring host must also restore its headless authoring state;
    /// the coordinator restores its held whole-batch preview from saved state.
    RestoreComputedFeatureRadius {
        expected: geosolve_sketch_features::ComputedFeatureEvaluationInput,
        feature: geosolve_sketch_features::ComputedFeatureId,
        radius: f64,
    },
    ClearComputedFeaturePreview,
    /// Preview of one explicit contact reseed along its named native parent.
    PreviewComputedFeatureContact {
        expected: geosolve_sketch_features::ComputedFeatureEvaluationInput,
        owner: geosolve_sketch_features::ComputedCornerRef,
        parent: ComputedFilletParentIndex,
        source: NativeCurveSpanSource,
        parameter: f64,
    },
    CommitComputedFeatureContact {
        expected: geosolve_sketch_features::ComputedFeatureEvaluationInput,
        owner: geosolve_sketch_features::ComputedCornerRef,
        parent: ComputedFilletParentIndex,
        source: NativeCurveSpanSource,
        parameter: f64,
    },
    RestoreComputedFeatureContact {
        expected: geosolve_sketch_features::ComputedFeatureEvaluationInput,
        owner: geosolve_sketch_features::ComputedCornerRef,
        parent: ComputedFilletParentIndex,
        source: NativeCurveSpanSource,
        parameter: f64,
    },
    ClearComputedFeatureContactPreview,
    /// The exact applicable branch alternative currently previewed by canvas
    /// hover or accessible focus. This is presentation state only; it carries
    /// no authority to mutate feature intent.
    FilletBranchPreviewChanged {
        target: Option<SceneFilletActionTarget>,
    },
    /// Requests one exact branch action that was already previewed and then
    /// revalidated through the same scene resolver used by hover/focus.
    CommitComputedFilletAction {
        target: SceneFilletActionTarget,
    },
    /// A complete typed construction commit envelope. Hosts apply `proposal`
    /// and its draft-frozen `role` together through the retained coordinator;
    /// detaching the geometry-only proposal would lose authoring intent.
    CommitConstruction {
        expected: SketchDesignIdentity,
        proposal: ConstructionProposal,
        role: GeometryRole,
    },
    /// Requests one atomic construction-plus-inference publication.
    ///
    /// The editor retains the terminal draft until the host reports the result
    /// through [`ConstraintEditor::acknowledge_construction_commit`].  A
    /// rejected publication therefore remains correction-ready rather than
    /// silently discarding the user's draft.
    CommitConstructionPlan {
        expected: Box<PreparedSketchInput>,
        token: ConstructionCommitToken,
        plan: ConstructionCommitPlan,
    },
    /// A non-authoritative staged construction preview.
    PreviewConstruction(ConstructionPreview),
    ClearConstructionPreview,
    /// Complete headless inference publication for the current draft sample.
    /// `None` explicitly clears every guide previously published by the editor.
    DraftInferenceChanged(Option<DraftInferenceResolution>),
}

/// Session-local identity for one pending atomic construction publication.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ConstructionCommitToken(u64);

impl ConstructionCommitToken {
    /// Stable raw value for presentation-adapter correlation and diagnostics.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A point operand used by a construction proposal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConstructionPoint {
    Existing {
        id: DesignPointId,
        /// The accepted visible position used when the operand was snapped.
        position: [f64; 2],
    },
    New([f64; 2]),
}

/// A typed, equation-free construction request.
///
/// Applying a proposal uses only public [`SketchDocument`] allocation APIs.  It is
/// deliberately separate from [`geosolve_sketch::DocumentEdit`], whose single-edit shape cannot
/// refer to identities allocated by preceding point/scalar creations.
#[derive(Clone, Debug, PartialEq)]
pub enum ConstructionProposal {
    Point {
        point: ConstructionPoint,
    },
    Line {
        start: ConstructionPoint,
        end: ConstructionPoint,
    },
    Polyline {
        points: Vec<ConstructionPoint>,
    },
    /// Geometry-only open or closed polyline recipe. A closed path reuses its
    /// first stored point and never appends a duplicate terminal control.
    PolylinePath {
        points: Vec<ConstructionPoint>,
        closed: bool,
    },
    /// One ordinary line authored symmetrically about a stored centre.
    MidpointLine {
        center: ConstructionPoint,
        endpoint: ConstructionPoint,
        opposite: ConstructionPoint,
    },
    Rectangle {
        first: [f64; 2],
        second: [f64; 2],
    },
    /// Four explicit shared-corner lines, optionally followed by one visible
    /// Construction diagonal whose midpoint is `center`.
    ///
    /// `points` is allocation order; `corners` indexes it in loop order. This
    /// keeps clicked operands first and derived corners later, so same-plan
    /// point slots remain deterministic even when a clicked point is reused.
    RectangleLoop {
        points: Vec<ConstructionPoint>,
        corners: [usize; 4],
        center: Option<usize>,
    },
    Circle {
        center: ConstructionPoint,
        radius: f64,
    },
    CounterClockwiseArc {
        center: ConstructionPoint,
        start: [f64; 2],
        end: [f64; 2],
    },
    /// Circular arc with explicit durable traversal branch.
    CircularArc {
        center: ConstructionPoint,
        start: [f64; 2],
        end: [f64; 2],
        sweep: DocumentArcSweep,
    },
    QuadraticBezier {
        controls: [ConstructionPoint; 3],
    },
    CubicBezier {
        controls: [ConstructionPoint; 4],
    },
    Ellipse {
        center: ConstructionPoint,
        major_axis_point: ConstructionPoint,
        minor_axis_ratio: f64,
    },
    /// Full ellipse whose first sampled pole remains the stored major-axis
    /// point while the centre is analytically derived from the diameter pair.
    AxisEndpointEllipse {
        major_axis_point: ConstructionPoint,
        center: ConstructionPoint,
        minor_axis_ratio: f64,
    },
    EllipticalArc {
        center: ConstructionPoint,
        major_axis_point: ConstructionPoint,
        minor_axis_ratio: f64,
        start_angle: f64,
        end_angle: f64,
        sweep: DocumentArcSweep,
    },
    /// Elliptical arc counterpart of [`Self::AxisEndpointEllipse`].
    AxisEndpointEllipticalArc {
        major_axis_point: ConstructionPoint,
        center: ConstructionPoint,
        minor_axis_ratio: f64,
        start_angle: f64,
        end_angle: f64,
        sweep: DocumentArcSweep,
    },
    RationalQuadraticConic {
        start: ConstructionPoint,
        weighted_middle: [f64; 2],
        middle_weight: f64,
        end: ConstructionPoint,
    },
    Parabola {
        vertex: ConstructionPoint,
        focus: ConstructionPoint,
        trim_start: f64,
        trim_end: f64,
    },
    Hyperbola {
        center: ConstructionPoint,
        transverse_axis_point: ConstructionPoint,
        semi_conjugate: f64,
        branch: DocumentHyperbolaBranch,
        trim_start: f64,
        trim_end: f64,
    },
    Nurbs {
        controls: Vec<ConstructionPoint>,
        options: NurbsConstructionOptions,
    },
}

/// Explicit authoring state for conic construction tools.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConicConstructionOptions {
    pub minor_axis_ratio: f64,
    /// Retained for source compatibility with hosts that store one common conic
    /// option record. Staged elliptical-arc authoring derives Start spatially.
    pub arc_start: f64,
    /// Retained for source compatibility with hosts that store one common conic
    /// option record. Staged elliptical-arc authoring derives End spatially.
    pub arc_end: f64,
    pub arc_sweep: DocumentArcSweep,
    pub middle_weight: f64,
    pub trim_start: f64,
    pub trim_end: f64,
    pub semi_conjugate: f64,
    pub hyperbola_branch: DocumentHyperbolaBranch,
}

impl Default for ConicConstructionOptions {
    fn default() -> Self {
        Self {
            minor_axis_ratio: 0.5,
            arc_start: 0.0,
            arc_end: std::f64::consts::FRAC_PI_2,
            arc_sweep: DocumentArcSweep::CounterClockwise,
            middle_weight: 1.0,
            trim_start: -1.0,
            trim_end: 1.0,
            semi_conjugate: 1.0,
            hyperbola_branch: DocumentHyperbolaBranch::Positive,
        }
    }
}

/// Explicit NURBS creation topology and homogeneous weights.
#[derive(Clone, Debug, PartialEq)]
pub struct NurbsConstructionOptions {
    pub form: DocumentBSplineForm,
    pub degree: u32,
    /// Empty means one unit weight per control. Otherwise the count must match.
    pub weights: Vec<f64>,
    pub gauge_index: usize,
}

impl Default for NurbsConstructionOptions {
    fn default() -> Self {
        Self {
            form: DocumentBSplineForm::Clamped,
            degree: 3,
            weights: Vec::new(),
            gauge_index: 0,
        }
    }
}

/// A typed non-authoritative construction preview.
///
/// Unlike [`ConstructionProposal`], this may describe an incomplete draft and is
/// never committable. Complete previews retain the exact proposal operands that
/// will be emitted on the terminal interaction.
#[derive(Clone, Debug, PartialEq)]
#[allow(
    clippy::large_enum_variant,
    reason = "a complete preview deliberately carries its exact typed proposal beside resolved geometry"
)]
pub enum ConstructionPreview {
    Complete {
        /// The exact proposal emitted by terminal completion.
        proposal: ConstructionProposal,
        /// Fully resolved presentation-neutral geometry from the same draft positions.
        geometry: ConstructionPreviewGeometry,
    },
    Anchor {
        position: [f64; 2],
    },
    ArcRadiusGuide {
        center: [f64; 2],
        start: [f64; 2],
    },
    /// Equation-free staged recipe guide for diameter chords, three-point
    /// circles/arcs and rotated rectangle baselines.
    GuidePolyline {
        points: Vec<[f64; 2]>,
        closed: bool,
    },
    /// Support ellipse established by the first two elliptical-arc clicks.
    /// Once present, `trim_start` is the radial inverse projection of the
    /// third spatial click rather than an independently persisted point.
    EllipticalArcSupport {
        center: [f64; 2],
        major_axis_point: [f64; 2],
        /// Public-domain-evaluated support ellipse polyline. Presentation
        /// adapters render these points without reconstructing conic equations.
        support_points: Vec<[f64; 2]>,
        trim_start: Option<[f64; 2]>,
    },
    ControlPolygon {
        kind: AdvancedConstructionKind,
        points: Vec<[f64; 2]>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdvancedConstructionKind {
    QuadraticBezier,
    CubicBezier,
    Ellipse,
    EllipticalArc,
    RationalQuadraticConic,
    Parabola,
    Hyperbola,
    Nurbs,
}

/// Fully resolved model-space geometry for a complete construction preview.
#[derive(Clone, Debug, PartialEq)]
pub enum ConstructionPreviewGeometry {
    Point {
        position: [f64; 2],
    },
    Polyline {
        points: Vec<[f64; 2]>,
    },
    Rectangle {
        first: [f64; 2],
        second: [f64; 2],
    },
    Circle {
        center: [f64; 2],
        radius: f64,
    },
    CounterClockwiseArc {
        center: [f64; 2],
        start: [f64; 2],
        end: [f64; 2],
        radius: f64,
        /// Explicit finite counterclockwise sweep in `(0, TAU)`.
        sweep_radians: f64,
        large_arc: bool,
    },
    CircularArc {
        center: [f64; 2],
        start: [f64; 2],
        end: [f64; 2],
        radius: f64,
        sweep_radians: f64,
        large_arc: bool,
        sweep: DocumentArcSweep,
    },
    AdvancedCurve {
        kind: AdvancedConstructionKind,
        control_points: Vec<[f64; 2]>,
        curve_points: Vec<[f64; 2]>,
    },
}

/// Persistent identities allocated by [`ConstructionProposal::apply`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConstructionResult {
    pub points: Vec<DesignPointId>,
    pub scalars: Vec<DesignScalarId>,
    pub curves: Vec<CurveId>,
}

impl ConstructionProposal {
    /// Applies this proposal atomically to a document using public construction APIs.
    ///
    /// A host normally calls this in `SketchDocumentSession::transact`, which adds
    /// solve validation and history atomicity. Direct use also leaves `document`
    /// unchanged when allocation or validation fails.
    ///
    /// # Errors
    ///
    /// Returns the public document validation/allocation error without mutation.
    pub fn apply(
        &self,
        document: &mut SketchDocument,
    ) -> Result<ConstructionResult, geosolve_sketch::DocumentError> {
        self.apply_with_role(document, GeometryRole::Profile)
    }

    /// Applies this proposal atomically and assigns every created curve the
    /// requested persistent geometry role. Standalone points remain role-neutral.
    ///
    /// # Errors
    ///
    /// Returns the public document validation/allocation error without mutation.
    pub fn apply_with_role(
        &self,
        document: &mut SketchDocument,
        role: GeometryRole,
    ) -> Result<ConstructionResult, geosolve_sketch::DocumentError> {
        let mut candidate = document.clone();
        let result = self.apply_to(&mut candidate)?;
        let helper_curve = matches!(
            self,
            Self::RectangleLoop {
                center: Some(_),
                ..
            }
        )
        .then(|| result.curves.last().copied())
        .flatten();
        if (role == GeometryRole::Construction && !result.curves.is_empty())
            || helper_curve.is_some()
        {
            let edits = result
                .curves
                .iter()
                .copied()
                .filter_map(|curve| {
                    let curve_role =
                        if role == GeometryRole::Construction || helper_curve == Some(curve) {
                            GeometryRole::Construction
                        } else {
                            return None;
                        };
                    Some(geosolve_sketch::GeometryRoleEdit::new(curve, curve_role))
                })
                .collect::<Vec<_>>();
            candidate.set_geometry_roles(&edits)?;
        }
        *document = candidate;
        Ok(result)
    }

    #[allow(clippy::too_many_lines)]
    fn apply_to(
        &self,
        document: &mut SketchDocument,
    ) -> Result<ConstructionResult, geosolve_sketch::DocumentError> {
        let mut result = ConstructionResult::default();
        let mut point =
            |operand: ConstructionPoint| -> Result<DesignPointId, geosolve_sketch::DocumentError> {
                match operand {
                    ConstructionPoint::Existing { id, .. } => {
                        document.point(id).ok_or_else(|| {
                            geosolve_sketch::DocumentError::InvalidField {
                                field: "construction point",
                                message: "existing point is absent from this document".into(),
                            }
                        })?;
                        Ok(id)
                    }
                    ConstructionPoint::New(position) => {
                        let id = document.add_point("draft point", position)?;
                        result.points.push(id);
                        Ok(id)
                    }
                }
            };
        match self {
            Self::Point { point: operand } => {
                let _ = point(*operand)?;
            }
            Self::Line { start, end } => {
                let branch_direction =
                    construction_branch_direction(start.position(), end.position())?;
                let start = point(*start)?;
                let end = point(*end)?;
                result.curves.push(document.add_curve(
                    "line",
                    CurveDefinition::Line {
                        start,
                        end,
                        branch_direction,
                    },
                )?);
            }
            Self::Polyline { points: operands } => {
                let directions = operands
                    .windows(2)
                    .map(|pair| {
                        construction_branch_direction(pair[0].position(), pair[1].position())
                    })
                    .collect::<Result<Vec<_>, geosolve_sketch::DocumentError>>()?;
                let points = operands
                    .iter()
                    .copied()
                    .map(&mut point)
                    .collect::<Result<Vec<_>, _>>()?;
                result.curves.push(document.add_curve(
                    "polyline",
                    CurveDefinition::Polyline {
                        points,
                        closed: false,
                        branch_directions: directions,
                    },
                )?);
            }
            Self::PolylinePath {
                points: operands,
                closed,
            } => {
                let segment_pairs = if *closed {
                    operands
                        .iter()
                        .copied()
                        .zip(operands.iter().copied().cycle().skip(1))
                        .take(operands.len())
                        .collect::<Vec<_>>()
                } else {
                    operands
                        .windows(2)
                        .map(|pair| (pair[0], pair[1]))
                        .collect::<Vec<_>>()
                };
                let directions = segment_pairs
                    .iter()
                    .map(|(start, end)| {
                        construction_branch_direction(start.position(), end.position())
                    })
                    .collect::<Result<Vec<_>, geosolve_sketch::DocumentError>>()?;
                let points = operands
                    .iter()
                    .copied()
                    .map(&mut point)
                    .collect::<Result<Vec<_>, _>>()?;
                result.curves.push(document.add_curve(
                    if *closed {
                        "closed polyline"
                    } else {
                        "polyline"
                    },
                    CurveDefinition::Polyline {
                        points,
                        closed: *closed,
                        branch_directions: directions,
                    },
                )?);
            }
            Self::MidpointLine {
                center,
                endpoint,
                opposite,
            } => {
                let branch_direction =
                    construction_branch_direction(opposite.position(), endpoint.position())?;
                let center = point(*center)?;
                let endpoint = point(*endpoint)?;
                let opposite = point(*opposite)?;
                result.curves.push(document.add_curve(
                    "midpoint line",
                    CurveDefinition::Line {
                        start: opposite,
                        end: endpoint,
                        branch_direction,
                    },
                )?);
                let _ = center;
            }
            Self::Rectangle { first, second } => {
                let origin = [first[0].min(second[0]), first[1].min(second[1])];
                let width = (second[0] - first[0]).abs();
                let height = (second[1] - first[1]).abs();
                let ids = document.add_rectangle("rectangle", origin, width, height)?;
                document.remove_with_owned_state(DocumentObjectId::Constraint(ids.anchor))?;
                for dimension in ids.dimensions {
                    document.remove_with_owned_state(DocumentObjectId::Dimension(dimension))?;
                }
                result.points.extend(ids.points);
                result.curves.extend(ids.curves);
            }
            Self::RectangleLoop {
                points: operands,
                corners,
                center,
            } => {
                if operands.len() > 8
                    || corners.iter().any(|index| *index >= operands.len())
                    || center.is_some_and(|index| index >= operands.len())
                {
                    return Err(geosolve_sketch::DocumentError::InvalidField {
                        field: "rectangle loop points",
                        message: "corner or centre occurrence is outside the point list".into(),
                    });
                }
                let point_ids = operands
                    .iter()
                    .copied()
                    .map(&mut point)
                    .collect::<Result<Vec<_>, _>>()?;
                for edge in 0..4 {
                    let start = point_ids[corners[edge]];
                    let end = point_ids[corners[(edge + 1) % 4]];
                    let branch_direction = construction_branch_direction(
                        operands[corners[edge]].position(),
                        operands[corners[(edge + 1) % 4]].position(),
                    )?;
                    result.curves.push(document.add_curve(
                        format!("rectangle edge {}", edge + 1),
                        CurveDefinition::Line {
                            start,
                            end,
                            branch_direction,
                        },
                    )?);
                }
                if center.is_some() {
                    let start = point_ids[corners[0]];
                    let end = point_ids[corners[2]];
                    let branch_direction = construction_branch_direction(
                        operands[corners[0]].position(),
                        operands[corners[2]].position(),
                    )?;
                    result.curves.push(document.add_curve(
                        "rectangle centre helper",
                        CurveDefinition::Line {
                            start,
                            end,
                            branch_direction,
                        },
                    )?);
                }
            }
            Self::Circle { center, radius } => {
                let center = point(*center)?;
                let radius = document.add_scalar(
                    "radius",
                    *radius,
                    ScalarUnit::Length,
                    ScalarDomain::Positive,
                )?;
                result.scalars.push(radius);
                result.curves.push(
                    document.add_curve("circle", CurveDefinition::Circle { center, radius })?,
                );
            }
            Self::CounterClockwiseArc { center, start, end } => {
                let center_position = center.position();
                let center = point(*center)?;
                let dx = start[0] - center_position[0];
                let dy = start[1] - center_position[1];
                let radius_value = dx.hypot(dy);
                let radius = document.add_scalar(
                    "arc radius",
                    radius_value,
                    ScalarUnit::Length,
                    ScalarDomain::Positive,
                )?;
                let start_angle = document.add_scalar(
                    "arc start",
                    dy.atan2(dx),
                    ScalarUnit::Angle,
                    ScalarDomain::Finite,
                )?;
                let end_angle = document.add_scalar(
                    "arc end",
                    (end[1] - center_position[1]).atan2(end[0] - center_position[0]),
                    ScalarUnit::Angle,
                    ScalarDomain::Finite,
                )?;
                result.scalars.extend([radius, start_angle, end_angle]);
                result.curves.push(document.add_curve(
                    "arc",
                    CurveDefinition::CircularArc {
                        center,
                        radius,
                        start_angle,
                        end_angle,
                        sweep: DocumentArcSweep::CounterClockwise,
                    },
                )?);
            }
            Self::CircularArc {
                center,
                start,
                end,
                sweep,
            } => {
                let center_position = center.position();
                let center = point(*center)?;
                let dx = start[0] - center_position[0];
                let dy = start[1] - center_position[1];
                let radius_value = dx.hypot(dy);
                let radius = document.add_scalar(
                    "arc radius",
                    radius_value,
                    ScalarUnit::Length,
                    ScalarDomain::Positive,
                )?;
                let start_angle = document.add_scalar(
                    "arc start",
                    dy.atan2(dx),
                    ScalarUnit::Angle,
                    ScalarDomain::Finite,
                )?;
                let end_angle = document.add_scalar(
                    "arc end",
                    (end[1] - center_position[1]).atan2(end[0] - center_position[0]),
                    ScalarUnit::Angle,
                    ScalarDomain::Finite,
                )?;
                result.scalars.extend([radius, start_angle, end_angle]);
                result.curves.push(document.add_curve(
                    "arc",
                    CurveDefinition::CircularArc {
                        center,
                        radius,
                        start_angle,
                        end_angle,
                        sweep: *sweep,
                    },
                )?);
            }
            Self::QuadraticBezier { controls } => {
                let controls = [
                    point(controls[0])?,
                    point(controls[1])?,
                    point(controls[2])?,
                ];
                result.curves.push(document.add_curve(
                    "quadratic Bezier",
                    CurveDefinition::QuadraticBezier { controls },
                )?);
            }
            Self::CubicBezier { controls } => {
                let controls = [
                    point(controls[0])?,
                    point(controls[1])?,
                    point(controls[2])?,
                    point(controls[3])?,
                ];
                result.curves.push(
                    document
                        .add_curve("cubic Bezier", CurveDefinition::CubicBezier { controls })?,
                );
            }
            Self::Ellipse {
                center,
                major_axis_point,
                minor_axis_ratio,
            } => {
                let center = point(*center)?;
                let major_axis_point = point(*major_axis_point)?;
                let minor_axis_ratio = document.add_scalar(
                    "ellipse minor-axis ratio",
                    *minor_axis_ratio,
                    ScalarUnit::Parameter,
                    ScalarDomain::Bounded {
                        lower: f64::from_bits(1),
                        upper: 1.0,
                    },
                )?;
                result.scalars.push(minor_axis_ratio);
                result.curves.push(document.add_curve(
                    "ellipse",
                    CurveDefinition::Ellipse {
                        center,
                        major_axis_point,
                        minor_axis_ratio,
                    },
                )?);
            }
            Self::AxisEndpointEllipse {
                major_axis_point,
                center,
                minor_axis_ratio,
            } => {
                let major_axis_point = point(*major_axis_point)?;
                let center = point(*center)?;
                let minor_axis_ratio = document.add_scalar(
                    "ellipse minor-axis ratio",
                    *minor_axis_ratio,
                    ScalarUnit::Parameter,
                    ScalarDomain::Bounded {
                        lower: f64::from_bits(1),
                        upper: 1.0,
                    },
                )?;
                result.scalars.push(minor_axis_ratio);
                result.curves.push(document.add_curve(
                    "axis-endpoint ellipse",
                    CurveDefinition::Ellipse {
                        center,
                        major_axis_point,
                        minor_axis_ratio,
                    },
                )?);
            }
            Self::EllipticalArc {
                center,
                major_axis_point,
                minor_axis_ratio,
                start_angle,
                end_angle,
                sweep,
            } => {
                let center = point(*center)?;
                let major_axis_point = point(*major_axis_point)?;
                let minor_axis_ratio = document.add_scalar(
                    "elliptical arc minor-axis ratio",
                    *minor_axis_ratio,
                    ScalarUnit::Parameter,
                    ScalarDomain::Bounded {
                        lower: f64::from_bits(1),
                        upper: 1.0,
                    },
                )?;
                let start_angle = document.add_scalar(
                    "elliptical arc start",
                    *start_angle,
                    ScalarUnit::Angle,
                    ScalarDomain::Finite,
                )?;
                let end_angle = document.add_scalar(
                    "elliptical arc end",
                    *end_angle,
                    ScalarUnit::Angle,
                    ScalarDomain::Finite,
                )?;
                result
                    .scalars
                    .extend([minor_axis_ratio, start_angle, end_angle]);
                result.curves.push(document.add_curve(
                    "elliptical arc",
                    CurveDefinition::EllipticalArc {
                        center,
                        major_axis_point,
                        minor_axis_ratio,
                        start_angle,
                        end_angle,
                        sweep: *sweep,
                    },
                )?);
            }
            Self::AxisEndpointEllipticalArc {
                major_axis_point,
                center,
                minor_axis_ratio,
                start_angle,
                end_angle,
                sweep,
            } => {
                let major_axis_point = point(*major_axis_point)?;
                let center = point(*center)?;
                let minor_axis_ratio = document.add_scalar(
                    "elliptical arc minor-axis ratio",
                    *minor_axis_ratio,
                    ScalarUnit::Parameter,
                    ScalarDomain::Bounded {
                        lower: f64::from_bits(1),
                        upper: 1.0,
                    },
                )?;
                let start_angle = document.add_scalar(
                    "elliptical arc start",
                    *start_angle,
                    ScalarUnit::Angle,
                    ScalarDomain::Finite,
                )?;
                let end_angle = document.add_scalar(
                    "elliptical arc end",
                    *end_angle,
                    ScalarUnit::Angle,
                    ScalarDomain::Finite,
                )?;
                result
                    .scalars
                    .extend([minor_axis_ratio, start_angle, end_angle]);
                result.curves.push(document.add_curve(
                    "axis-endpoint elliptical arc",
                    CurveDefinition::EllipticalArc {
                        center,
                        major_axis_point,
                        minor_axis_ratio,
                        start_angle,
                        end_angle,
                        sweep: *sweep,
                    },
                )?);
            }
            Self::RationalQuadraticConic {
                start,
                weighted_middle,
                middle_weight,
                end,
            } => {
                let start = point(*start)?;
                let end = point(*end)?;
                let middle_weight = document.add_scalar(
                    "rational conic middle weight",
                    *middle_weight,
                    ScalarUnit::Parameter,
                    ScalarDomain::Bounded {
                        lower: MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
                        upper: f64::MAX,
                    },
                )?;
                result.scalars.push(middle_weight);
                result.curves.push(document.add_curve(
                    "rational quadratic conic",
                    CurveDefinition::RationalQuadraticConic {
                        start,
                        weighted_middle: *weighted_middle,
                        middle_weight,
                        end,
                    },
                )?);
            }
            Self::Parabola {
                vertex,
                focus,
                trim_start,
                trim_end,
            } => {
                let vertex = point(*vertex)?;
                let focus = point(*focus)?;
                let trim_start = document.add_scalar(
                    "parabola trim start",
                    *trim_start,
                    ScalarUnit::Parameter,
                    ScalarDomain::Finite,
                )?;
                let trim_end = document.add_scalar(
                    "parabola trim end",
                    *trim_end,
                    ScalarUnit::Parameter,
                    ScalarDomain::Finite,
                )?;
                result.scalars.extend([trim_start, trim_end]);
                result.curves.push(document.add_curve(
                    "parabola",
                    CurveDefinition::ParabolaSegment {
                        vertex,
                        focus,
                        trim_start,
                        trim_end,
                    },
                )?);
            }
            Self::Hyperbola {
                center,
                transverse_axis_point,
                semi_conjugate,
                branch,
                trim_start,
                trim_end,
            } => {
                let center = point(*center)?;
                let transverse_axis_point = point(*transverse_axis_point)?;
                let semi_conjugate = document.add_scalar(
                    "hyperbola semi-conjugate",
                    *semi_conjugate,
                    ScalarUnit::Length,
                    ScalarDomain::Positive,
                )?;
                let trim_start = document.add_scalar(
                    "hyperbola trim start",
                    *trim_start,
                    ScalarUnit::Parameter,
                    ScalarDomain::Finite,
                )?;
                let trim_end = document.add_scalar(
                    "hyperbola trim end",
                    *trim_end,
                    ScalarUnit::Parameter,
                    ScalarDomain::Finite,
                )?;
                result
                    .scalars
                    .extend([semi_conjugate, trim_start, trim_end]);
                result.curves.push(document.add_curve(
                    "hyperbola",
                    CurveDefinition::HyperbolaSegment {
                        center,
                        transverse_axis_point,
                        semi_conjugate,
                        branch: *branch,
                        trim_start,
                        trim_end,
                    },
                )?);
            }
            Self::Nurbs { controls, options } => {
                let control_count = controls.len();
                validate_nurbs_for_controls(options, control_count)?;
                let mut control_ids = Vec::with_capacity(control_count);
                for control in controls {
                    control_ids.push(point(*control)?);
                }
                let values = if options.weights.is_empty() {
                    vec![1.0; control_count]
                } else {
                    options.weights.clone()
                };
                let mut weights = Vec::with_capacity(values.len());
                for (index, value) in values.into_iter().enumerate() {
                    let weight = document.add_scalar(
                        format!("NURBS weight {}", index + 1),
                        value,
                        ScalarUnit::Parameter,
                        ScalarDomain::Positive,
                    )?;
                    result.scalars.push(weight);
                    weights.push(weight);
                }
                let (knots, span_ids, next_span_id) =
                    nurbs_topology(options.form, options.degree, control_count)?;
                result.curves.push(document.add_curve(
                    "NURBS",
                    CurveDefinition::Nurbs {
                        form: options.form,
                        degree: options.degree,
                        controls: control_ids,
                        gauge_weight: weights[options.gauge_index],
                        weights,
                        knots,
                        span_ids,
                        next_span_id,
                    },
                )?);
            }
        }
        Ok(result)
    }
}

impl ConstructionPoint {
    fn position(self) -> [f64; 2] {
        match self {
            Self::Existing { position, .. } | Self::New(position) => position,
        }
    }
}

fn construction_branch_direction(
    first: [f64; 2],
    second: [f64; 2],
) -> Result<[f64; 2], geosolve_sketch::DocumentError> {
    let delta = [second[0] - first[0], second[1] - first[1]];
    let norm = delta[0].hypot(delta[1]);
    if !norm.is_finite() || norm <= 0.0 {
        return Err(geosolve_sketch::DocumentError::InvalidField {
            field: "construction branch direction",
            message: "endpoints must define a finite nonzero direction".into(),
        });
    }
    Ok([delta[0] / norm, delta[1] / norm])
}

fn point_role_incidence(
    document: &SketchDocument,
) -> std::collections::BTreeMap<DesignPointId, ScenePointRoleIncidence> {
    let mut incidence = document
        .points()
        .iter()
        .map(|point| (point.id, ScenePointRoleIncidence::default()))
        .collect::<std::collections::BTreeMap<_, _>>();
    for curve in document.curves() {
        let role = document.geometry_role(curve.id).unwrap_or_default();
        for point in curve_definition_points(&curve.definition) {
            if let Some(entry) = incidence.get_mut(&point) {
                match role {
                    GeometryRole::Profile => entry.profile = true,
                    GeometryRole::Construction => entry.construction = true,
                }
            }
        }
    }
    // A point with no curve incidence remains ordinary profile authoring state.
    // This keeps standalone sketch points usable without inventing a persistent
    // point role.
    for value in incidence.values_mut() {
        if !value.profile && !value.construction {
            value.profile = true;
        }
    }
    incidence
}

fn curve_definition_points(definition: &CurveDefinition) -> Vec<DesignPointId> {
    match definition {
        CurveDefinition::Line { start, end, .. }
        | CurveDefinition::RationalQuadraticConic { start, end, .. } => vec![*start, *end],
        CurveDefinition::Polyline { points, .. }
        | CurveDefinition::BSpline {
            controls: points, ..
        }
        | CurveDefinition::Nurbs {
            controls: points, ..
        } => points.clone(),
        CurveDefinition::Circle { center, .. } | CurveDefinition::CircularArc { center, .. } => {
            vec![*center]
        }
        CurveDefinition::QuadraticBezier { controls } => controls.to_vec(),
        CurveDefinition::CubicBezier { controls } => controls.to_vec(),
        CurveDefinition::Ellipse {
            center,
            major_axis_point,
            ..
        }
        | CurveDefinition::EllipticalArc {
            center,
            major_axis_point,
            ..
        } => vec![*center, *major_axis_point],
        CurveDefinition::ParabolaSegment { vertex, focus, .. } => vec![*vertex, *focus],
        CurveDefinition::HyperbolaSegment {
            center,
            transverse_axis_point,
            ..
        } => vec![*center, *transverse_axis_point],
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PointGesture {
    epoch: u64,
    pointer_id: u64,
    point: DesignPointId,
    origin: ScreenPoint,
    model_offset: [f64; 2],
    moved: bool,
    latest_request: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CurveControlGesture {
    pointer_id: u64,
    control: DocumentCurveControlId,
    owner: CurveSpan,
    expected: SketchDesignIdentity,
    accepted_revision: u64,
    viewport: Viewport,
    origin: ScreenPoint,
    origin_model: [f64; 2],
    model_position: [f64; 2],
    model_offset: [f64; 2],
    rail: Option<SceneCurveControlRail>,
    moved: bool,
    last_sampled_position: Option<[f64; 2]>,
    latest_request: Option<u64>,
    last_valid_request: Option<(u64, [f64; 2])>,
}

#[derive(Clone, Debug, PartialEq)]
struct AnnotationGesture {
    pointer_id: u64,
    key: AnnotationLayoutKey,
    accepted_revision: u64,
    design_identity: SketchDesignIdentity,
    viewport: Viewport,
    origin: ScreenPoint,
    original: Option<AnnotationPlacement>,
    automatic: AnnotationPlacement,
    preview: Option<AnnotationPlacement>,
    moved: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct FeatureRadiusGesture {
    pointer_id: u64,
    owner: geosolve_sketch_features::ComputedCornerRef,
    expected: geosolve_sketch_features::ComputedFeatureEvaluationInput,
    origin: ScreenPoint,
    origin_model: [f64; 2],
    model_derivative: [f64; 2],
    viewport: Viewport,
    moved: bool,
    origin_radius: f64,
    last_sampled_radius: Option<f64>,
    last_sampled_position: Option<ScreenPoint>,
    last_requested_radius: Option<f64>,
    last_accepted_radius: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct FeatureContactGesture {
    pointer_id: u64,
    owner: geosolve_sketch_features::ComputedCornerRef,
    parent: ComputedFilletParentIndex,
    source: NativeCurveSpanSource,
    expected: geosolve_sketch_features::ComputedFeatureEvaluationInput,
    viewport: Viewport,
    origin: ScreenPoint,
    moved: bool,
    origin_parameter: f64,
    last_sampled_parameter: Option<f64>,
    last_sampled_position: Option<ScreenPoint>,
    last_requested_parameter: Option<f64>,
    last_accepted_parameter: Option<f64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProjectedDragRequestDisposition {
    Current { gesture_epoch: u64 },
    Stale,
    Untracked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CurveControlPreviewRequestDisposition {
    Current,
    Stale,
    Untracked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PointGestureSnapshot {
    pub(crate) epoch: u64,
    pub(crate) pointer_id: u64,
    pub(crate) point: DesignPointId,
}

/// Active drafting tool. `Select` preserves the M40.2 selection behavior.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum EditorTool {
    #[default]
    Select,
    /// Places one point. Confirming an inferred existing point reuses that
    /// persistent identity as a history-neutral no-op: it allocates no point
    /// and creates no redundant coincidence source.
    Point,
    Line,
    Polyline,
    Rectangle,
    Circle,
    CounterClockwiseArc,
    QuadraticBezier,
    CubicBezier,
    Ellipse,
    EllipticalArc,
    RationalQuadraticConic,
    Parabola,
    Hyperbola,
    Nurbs,
}

/// Semantic operand currently requested by an exact geometry recipe.
///
/// Hosts use this vocabulary for prompts and accessibility.  It deliberately
/// describes authoring intent rather than exposing the draft's private storage
/// layout or asking a renderer to infer meaning from a coordinate count.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GeometryDraftStage {
    Point,
    Start,
    End,
    Center,
    Corner,
    AdjacentCorner,
    OppositeCorner,
    SideMidpoint,
    DiameterStart,
    DiameterEnd,
    ThroughPoint,
    SourceEndpoint,
    MajorAxisEndpoint,
    OppositeAxisEndpoint,
    MinorExtent,
    ControlPoint,
    Vertex,
    Focus,
    TransverseAxisEndpoint,
    ConjugateExtent,
    TrimStart,
    TrimEnd,
}

/// Typed live quantity published by a geometry draft.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum GeometryDraftMeasurement {
    Length(f64),
    Radius(f64),
    Diameter(f64),
    AngleRadians(f64),
    Ratio(f64),
    WidthHeight { width: f64, height: f64 },
    ControlCount(usize),
}

/// Explicit discrete branch state retained by a draft.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GeometryDraftBranch {
    pub sweep: Option<DocumentArcSweep>,
    pub hyperbola: Option<DocumentHyperbolaBranch>,
}

/// Draft-local reason that the current recipe cannot advance or publish.
///
/// These issues describe only the disposable authoring draft. They never
/// replace the accepted document's retained problem state, and disappear as
/// soon as the user corrects, steps back, cancels, or completes the draft.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GeometryDraftIssue {
    /// The latest terminal sample would create coincident, collinear,
    /// zero-length, zero-sweep, or otherwise non-finite geometry.
    InvalidTerminalGeometry,
    /// A snapped operand cannot be represented by the completed recipe's
    /// ordinary durable relations.
    IncompatibleConstraintIntent,
    /// An explicit Finish action needs additional controls or corrected
    /// variant options before it can publish.
    CannotFinish,
    /// The complete atomic plan was rejected by retained solve/validation or
    /// compare-and-swap publication.
    ConstructionRejected,
}

/// Read-only semantic state for the active exact geometry recipe.
#[derive(Clone, Debug, PartialEq)]
pub struct GeometryDraftStatus {
    pub variant: GeometryToolVariant,
    pub stage: GeometryDraftStage,
    pub completed_stages: usize,
    /// Fixed total stage count, or `None` for variable-length recipes.
    pub required_stages: Option<usize>,
    pub can_finish: bool,
    pub regularized: bool,
    pub branch: GeometryDraftBranch,
    pub measurements: Vec<GeometryDraftMeasurement>,
    /// A recoverable issue owned by this disposable draft, never by the
    /// accepted document's global Problems state.
    pub issue: Option<GeometryDraftIssue>,
}

#[derive(Clone, Debug)]
struct Draft {
    tool: EditorTool,
    variant: GeometryToolVariant,
    /// `false` preserves the legacy `EditorTool` gesture contract for hosts
    /// that have not opted into the exact M78 recipe API.
    exact_variant: bool,
    geometry_role: GeometryRole,
    prepared_input: Option<PreparedSketchInput>,
    pointer_id: u64,
    points: Vec<ConstructionPoint>,
    positions: Vec<[f64; 2]>,
    confirmed_inference: Vec<ConfirmedDraftInference>,
    regularized: bool,
    closed: bool,
    tangent_source: Option<TangentArcSource>,
    conic_options: ConicConstructionOptions,
    nurbs_options: NurbsConstructionOptions,
}

#[derive(Clone, Copy, Debug)]
struct TangentArcSource {
    endpoint: DocumentEndpointRef,
    contact: DocumentContactSeed,
    domain: ContactDomain,
    position: [f64; 2],
    outgoing_tangent: [f64; 2],
    orientation: TangentOrientation,
}

#[derive(Clone, Debug)]
struct ConfirmedDraftInference {
    candidate_id: DraftInferenceCandidateId,
    stage_index: usize,
    relations: Vec<DraftInferenceRelation>,
    references: Vec<DraftReferenceAnchor>,
}

#[derive(Clone, Debug)]
struct PendingConstructionCommit {
    token: ConstructionCommitToken,
    expected: Box<PreparedSketchInput>,
    plan: ConstructionCommitPlan,
    recovery_inference_engine: DraftInferenceEngine,
}

#[derive(Clone, Debug)]
struct ResolvedDraftStage {
    operand: ConstructionPoint,
    position: [f64; 2],
    confirmed: Option<ConfirmedDraftInference>,
    resolution: Option<DraftInferenceResolution>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct AnnotationHoverContext {
    owner: SelectionItem,
    origin: ScreenPoint,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ResolvedSelectPointerTarget {
    FilletRadius(Hit),
    CurveControl(SceneCurveControlHit),
    Geometry(Hit),
    Annotation {
        occurrence: SceneAnnotationOccurrence,
        distance_pixels: f64,
    },
}

impl ResolvedSelectPointerTarget {
    const fn hit(self) -> Hit {
        match self {
            Self::FilletRadius(hit) | Self::Geometry(hit) => hit,
            Self::CurveControl(hit) => Hit {
                item: SelectionItem::Curve(hit.owner()),
                distance_pixels: hit.distance_pixels(),
                curve_parameter: None,
                geometry: None,
            },
            Self::Annotation {
                occurrence,
                distance_pixels,
            } => Hit {
                item: occurrence.item,
                distance_pixels,
                curve_parameter: None,
                geometry: None,
            },
        }
    }

    const fn hover_target(self) -> EditorHoverTarget {
        match self {
            Self::FilletRadius(hit) | Self::Geometry(hit) => EditorHoverTarget::Geometry(hit.item),
            Self::CurveControl(SceneCurveControlHit::PointAlias { point, .. }) => {
                EditorHoverTarget::Geometry(SelectionItem::Point(point))
            }
            Self::CurveControl(SceneCurveControlHit::Direct { control, owner, .. }) => {
                EditorHoverTarget::CurveControl { control, owner }
            }
            Self::Annotation { occurrence, .. } => EditorHoverTarget::Annotation(occurrence),
        }
    }
}

/// Headless deterministic selection and point-gesture state machine.
#[derive(Clone, Debug)]
pub struct ConstraintEditor {
    selection: Vec<SelectionItem>,
    hover_target: Option<EditorHoverTarget>,
    hover_context: Option<AnnotationHoverContext>,
    curve_pick_parameters: Vec<(CurveSpan, f64)>,
    curve_pick_origins: Vec<(CurveSpan, SceneCurveOrigin)>,
    geometry_policy: GeometryInteractionPolicy,
    authoring_geometry_role: GeometryRole,
    pick_tolerance: PickTolerance,
    drag_threshold_pixels: f64,
    point_gesture: Option<PointGesture>,
    curve_control_gesture: Option<CurveControlGesture>,
    annotation_gesture: Option<AnnotationGesture>,
    annotation_layout: AnnotationLayoutState,
    feature_radius_gesture: Option<FeatureRadiusGesture>,
    feature_contact_gesture: Option<FeatureContactGesture>,
    computed_fillet_continuation_status: Option<ComputedFilletContinuationStatus>,
    fillet_branch_preview: Option<SceneFilletActionTarget>,
    tool: EditorTool,
    geometry_tool_variant: Option<GeometryToolVariant>,
    exact_geometry_tool: bool,
    conic_options: ConicConstructionOptions,
    nurbs_options: NurbsConstructionOptions,
    draft: Option<Draft>,
    draft_issue: Option<GeometryDraftIssue>,
    last_valid_drag_preview: Option<(u64, u64, u64, DesignPointId, [f64; 2])>,
    next_point_gesture_epoch: u64,
    next_projection_request: u64,
    draft_inference_engine: DraftInferenceEngine,
    draft_inference_resolution: Option<DraftInferenceResolution>,
    pending_construction_commit: Option<PendingConstructionCommit>,
    next_construction_commit_token: u64,
}

impl Default for ConstraintEditor {
    fn default() -> Self {
        Self {
            selection: Vec::new(),
            hover_target: None,
            hover_context: None,
            curve_pick_parameters: Vec::new(),
            curve_pick_origins: Vec::new(),
            geometry_policy: GeometryInteractionPolicy::default(),
            authoring_geometry_role: GeometryRole::Profile,
            pick_tolerance: PickTolerance::default(),
            drag_threshold_pixels: 3.0,
            point_gesture: None,
            curve_control_gesture: None,
            annotation_gesture: None,
            annotation_layout: AnnotationLayoutState::default(),
            feature_radius_gesture: None,
            feature_contact_gesture: None,
            computed_fillet_continuation_status: None,
            fillet_branch_preview: None,
            tool: EditorTool::Select,
            geometry_tool_variant: None,
            exact_geometry_tool: false,
            conic_options: ConicConstructionOptions::default(),
            nurbs_options: NurbsConstructionOptions::default(),
            draft: None,
            draft_issue: None,
            last_valid_drag_preview: None,
            next_point_gesture_epoch: 0,
            next_projection_request: 0,
            draft_inference_engine: DraftInferenceEngine::default(),
            draft_inference_resolution: None,
            pending_construction_commit: None,
            next_construction_commit_token: 1,
        }
    }
}

impl ConstraintEditor {
    /// Creates an editor with explicit finite interaction policy.
    ///
    /// # Errors
    ///
    /// Rejects negative or non-finite thresholds.
    pub fn new(
        pick_tolerance: PickTolerance,
        drag_threshold_pixels: f64,
    ) -> Result<Self, EditorError> {
        if !pick_tolerance.is_valid()
            || !drag_threshold_pixels.is_finite()
            || drag_threshold_pixels < 0.0
        {
            return Err(EditorError::InvalidTolerance);
        }
        Ok(Self {
            pick_tolerance,
            drag_threshold_pixels,
            ..Self::default()
        })
    }

    /// Selects a drafting tool, cancelling incomplete drafts and point gestures without edits.
    ///
    /// A moved gesture emits [`EditorEffect::ClearPointPreview`] even when every
    /// projection was rejected, so hosts also close retained continuation state.
    pub fn activate_tool(&mut self, tool: EditorTool) -> Vec<EditorEffect> {
        self.activate_projected_tool(
            tool,
            GeometryToolVariant::default_for_editor_tool(tool),
            false,
        )
    }

    /// Selects an exact geometry-authoring recipe while retaining its coarse
    /// [`EditorTool`] compatibility projection.
    pub fn activate_geometry_tool(&mut self, variant: GeometryToolVariant) -> Vec<EditorEffect> {
        self.activate_projected_tool(variant.editor_tool(), Some(variant), true)
    }

    fn activate_projected_tool(
        &mut self,
        tool: EditorTool,
        geometry_tool_variant: Option<GeometryToolVariant>,
        exact_geometry_tool: bool,
    ) -> Vec<EditorEffect> {
        if self.pending_construction_commit.is_some() {
            return Vec::new();
        }
        let leaving_select = self.tool == EditorTool::Select && tool != EditorTool::Select;
        self.tool = tool;
        self.geometry_tool_variant = geometry_tool_variant;
        self.exact_geometry_tool = exact_geometry_tool;
        let mut effects = self.cancel_draft();
        effects.extend(self.cancel_point_gesture());
        effects.extend(self.cancel_curve_control_gesture());
        effects.extend(self.cancel_annotation_gesture());
        effects.extend(self.cancel_feature_radius_gesture());
        effects.extend(self.cancel_feature_contact_gesture());
        effects.extend(self.clear_fillet_branch_preview());
        if leaving_select {
            effects.extend(self.invalidate_pointer_context());
        }
        effects
    }

    /// Returns the active tool.
    #[must_use]
    pub const fn tool(&self) -> EditorTool {
        self.tool
    }

    /// Returns the exact active geometry recipe, or `None` in Select mode.
    #[must_use]
    pub const fn geometry_tool_variant(&self) -> Option<GeometryToolVariant> {
        self.geometry_tool_variant
    }

    /// Returns the complete session-local geometry interaction policy.
    #[must_use]
    pub const fn geometry_interaction_policy(&self) -> GeometryInteractionPolicy {
        self.geometry_policy
    }

    /// Atomically replaces the complete geometry interaction policy and cancels
    /// interaction admitted under the previous policy. Durable selection
    /// identities remain intact.
    pub fn set_geometry_interaction_policy(
        &mut self,
        policy: GeometryInteractionPolicy,
    ) -> Vec<EditorEffect> {
        if self.pending_construction_commit.is_some() {
            return Vec::new();
        }
        if self.geometry_policy == policy {
            return Vec::new();
        }
        self.geometry_policy = policy;
        self.cancel_interaction_for_geometry_policy_change()
    }

    /// Replaces the ordinary canvas pick scope and cancels interaction admitted
    /// under the previous policy. Durable selection identities remain intact.
    pub fn set_geometry_pick_scope(&mut self, scope: GeometryPickScope) -> Vec<EditorEffect> {
        self.set_geometry_interaction_policy(GeometryInteractionPolicy {
            scope,
            ..self.geometry_policy
        })
    }

    /// Replaces explicit/implicit construction visibility and cancels interaction
    /// admitted under the previous policy. Existing selection identities remain
    /// intact and recover when the geometry is shown again.
    pub fn set_geometry_visibility(&mut self, visibility: GeometryVisibility) -> Vec<EditorEffect> {
        self.set_geometry_interaction_policy(GeometryInteractionPolicy {
            visibility,
            ..self.geometry_policy
        })
    }

    fn cancel_interaction_for_geometry_policy_change(&mut self) -> Vec<EditorEffect> {
        let mut effects = self.cancel_draft();
        effects.extend(self.cancel_point_gesture());
        effects.extend(self.cancel_curve_control_gesture());
        effects.extend(self.cancel_annotation_gesture());
        effects.extend(self.cancel_feature_radius_gesture());
        effects.extend(self.cancel_feature_contact_gesture());
        effects.extend(self.clear_fillet_branch_preview());
        effects.extend(self.clear_hover_for_geometry_policy_change());
        effects
    }

    fn clear_hover_for_geometry_policy_change(&mut self) -> Vec<EditorEffect> {
        self.invalidate_pointer_context()
    }

    /// Returns the role assigned atomically to curves created by the active
    /// drawing workflow. Standalone points remain role-neutral.
    #[must_use]
    pub const fn authoring_geometry_role(&self) -> GeometryRole {
        self.authoring_geometry_role
    }

    /// Chooses the role for subsequently started drawing workflows.
    pub fn set_authoring_geometry_role(&mut self, role: GeometryRole) {
        if self.pending_construction_commit.is_none() {
            self.authoring_geometry_role = role;
        }
    }

    /// Returns the reusable semantic drafting-inference policy.
    #[must_use]
    pub const fn draft_inference_policy(&self) -> DraftInferencePolicy {
        self.draft_inference_engine.policy()
    }

    /// Replaces drafting inference policy and clears state acquired under the
    /// previous policy.
    ///
    /// # Errors
    ///
    /// Returns [`DraftInferenceError::InvalidPolicy`] without changing policy.
    pub fn set_draft_inference_policy(
        &mut self,
        policy: DraftInferencePolicy,
    ) -> Result<Vec<EditorEffect>, DraftInferenceError> {
        policy.validate()?;
        if self.pending_construction_commit.is_some() {
            return Ok(Vec::new());
        }
        self.draft_inference_engine.set_policy(policy)?;
        Ok(self.clear_draft_inference_publication())
    }

    /// Current complete headless inference publication, if any.
    #[must_use]
    pub fn draft_inference_resolution(&self) -> Option<&DraftInferenceResolution> {
        self.draft_inference_resolution.as_ref()
    }

    /// Invalidates camera/scene-bound inference and pointer-context presentation.
    ///
    /// Presentation adapters call this immediately when the viewport changes;
    /// waiting for another pointer sample could otherwise leave stale guides on
    /// screen. The construction draft itself is preserved.
    pub fn invalidate_draft_inference(&mut self) -> Vec<EditorEffect> {
        self.draft_inference_engine.clear_session();
        if let Some(pending) = self.pending_construction_commit.as_mut() {
            pending.recovery_inference_engine.clear_session();
        }
        let mut effects = self.cancel_curve_control_gesture();
        effects.extend(self.cancel_annotation_gesture());
        effects.extend(self.invalidate_pointer_context());
        effects.extend(self.clear_draft_inference_publication());
        effects
    }

    fn clear_draft_inference_publication(&mut self) -> Vec<EditorEffect> {
        self.draft_inference_resolution
            .take()
            .map(|_| EditorEffect::DraftInferenceChanged(None))
            .into_iter()
            .collect()
    }

    /// Replaces explicit conic authoring values and updates a retained conic draft.
    ///
    /// # Errors
    ///
    /// Rejects non-finite or out-of-domain values without changing editor state.
    pub fn set_conic_options(
        &mut self,
        options: ConicConstructionOptions,
    ) -> Result<(), EditorError> {
        validate_conic_options(options)?;
        if self.pending_construction_commit.is_some() {
            return Ok(());
        }
        self.conic_options = options;
        if let Some(draft) = self.draft.as_mut()
            && is_conic_tool(draft.tool)
        {
            draft.conic_options = options;
            self.draft_issue = None;
        }
        Ok(())
    }

    #[must_use]
    pub const fn conic_options(&self) -> ConicConstructionOptions {
        self.conic_options
    }

    /// Replaces explicit NURBS topology/weight authoring state.
    ///
    /// # Errors
    ///
    /// Rejects zero degree, non-positive/non-finite weights, or a gauge index
    /// outside a non-empty weight list.
    pub fn set_nurbs_options(
        &mut self,
        options: NurbsConstructionOptions,
    ) -> Result<(), EditorError> {
        validate_nurbs_options(&options)?;
        if self.pending_construction_commit.is_some() {
            return Ok(());
        }
        self.nurbs_options = options.clone();
        if let Some(draft) = self.draft.as_mut()
            && draft.tool == EditorTool::Nurbs
        {
            draft.nurbs_options = options;
            self.draft_issue = None;
        }
        Ok(())
    }

    #[must_use]
    pub fn nurbs_options(&self) -> &NurbsConstructionOptions {
        &self.nurbs_options
    }

    #[must_use]
    pub fn selection(&self) -> &[SelectionItem] {
        &self.selection
    }

    /// Recomputes the selected-only native curve cage on one accepted scene.
    ///
    /// The scene remains empty outside Select mode, for zero/multiple selections,
    /// for non-curve owners, and for stale or wholly non-editable native curves.
    /// Presentation adapters call this after composing the accepted scene and do
    /// not reconstruct family controls from SVG geometry.
    ///
    /// # Errors
    ///
    /// Returns a typed accepted-domain control-enumeration failure without
    /// publishing a partial cage.
    pub fn populate_curve_controls(&self, scene: &mut EditorScene) -> Result<(), EditorError> {
        let owner = (self.tool == EditorTool::Select && self.selection.len() == 1)
            .then(|| match self.selection[0] {
                SelectionItem::Curve(span) => Some(span),
                _ => None,
            })
            .flatten();
        scene.set_selected_curve_controls(owner)
    }

    /// Returns presentation-only manual annotation placement.
    #[must_use]
    pub const fn annotation_layout(&self) -> &AnnotationLayoutState {
        &self.annotation_layout
    }

    /// Returns committed layout plus the current uncommitted drag preview.
    ///
    /// Hosts use this only while rebuilding a scene. Persistence must continue
    /// to consume [`Self::annotation_layout`], so a canceled or interrupted
    /// gesture can never reappear after reload.
    #[must_use]
    pub fn annotation_layout_for_scene(&self) -> AnnotationLayoutState {
        let mut layout = self.annotation_layout.clone();
        if let Some(gesture) = &self.annotation_gesture
            && let Some(preview) = gesture.preview
        {
            layout.insert(gesture.key, preview);
        }
        layout
    }

    /// Restores a disposable layout cache after independently validating it.
    pub fn restore_annotation_layout(&mut self, layout: AnnotationLayoutState) {
        self.annotation_gesture = None;
        self.annotation_layout = layout;
    }

    /// Resets the selected movable annotations to deterministic automatic placement.
    pub fn reset_selected_annotation_layout(&mut self) -> bool {
        let selection = self.selection.clone();
        let canceled_preview = self
            .annotation_gesture
            .as_ref()
            .is_some_and(|gesture| selection.contains(&gesture.key.item));
        if canceled_preview {
            self.annotation_gesture = None;
        }
        selection
            .into_iter()
            .fold(canceled_preview, |changed, item| {
                self.annotation_layout.remove_item(item) || changed
            })
    }

    /// Resets every manual annotation placement.
    pub fn reset_all_annotation_layout(&mut self) -> bool {
        let canceled_preview = self.annotation_gesture.take().is_some();
        self.annotation_layout.clear() || canceled_preview
    }

    /// Returns the exact pointer currently owned by an editor gesture.
    ///
    /// Presentation adapters use this only to mirror platform pointer capture;
    /// the editor's own pointer-ID checks remain authoritative.
    #[must_use]
    pub const fn active_pointer_gesture(&self) -> Option<ActivePointerGesture> {
        if let Some(gesture) = self.feature_contact_gesture {
            return Some(ActivePointerGesture {
                pointer_id: gesture.pointer_id,
                kind: ActivePointerGestureKind::FilletContact,
            });
        }
        if let Some(gesture) = self.feature_radius_gesture {
            return Some(ActivePointerGesture {
                pointer_id: gesture.pointer_id,
                kind: ActivePointerGestureKind::FilletRadius,
            });
        }
        if let Some(gesture) = self.curve_control_gesture {
            return Some(ActivePointerGesture {
                pointer_id: gesture.pointer_id,
                kind: ActivePointerGestureKind::CurveControl,
            });
        }
        if let Some(gesture) = &self.annotation_gesture {
            return Some(ActivePointerGesture {
                pointer_id: gesture.pointer_id,
                kind: ActivePointerGestureKind::Annotation,
            });
        }
        if let Some(gesture) = self.point_gesture {
            return Some(ActivePointerGesture {
                pointer_id: gesture.pointer_id,
                kind: ActivePointerGestureKind::Point,
            });
        }
        None
    }

    /// Returns the typed limit for the active Fillet gesture's exact rejected
    /// sample. The last accepted current preview remains owned by the coordinator.
    #[must_use]
    pub fn computed_fillet_continuation_status(&self) -> Option<&ComputedFilletContinuationStatus> {
        self.computed_fillet_continuation_status.as_ref()
    }

    /// Returns the exact scene-stamped Fillet branch alternative currently
    /// previewed by canvas hover or accessible focus.
    #[must_use]
    pub const fn fillet_branch_preview(&self) -> Option<SceneFilletActionTarget> {
        self.fillet_branch_preview
    }

    /// Enters or updates Fillet branch preview through the shared action resolver.
    ///
    /// Invalid, disabled, stale, spoofed or occluded canvas input clears an old
    /// preview and never emits a feature mutation.
    pub fn preview_fillet_action(
        &mut self,
        scene: &EditorScene,
        input: SceneFilletActionInput,
    ) -> Vec<EditorEffect> {
        let target = (self.tool == EditorTool::Select && self.active_pointer_gesture().is_none())
            .then(|| {
                scene.resolve_fillet_action_with_policy(
                    input,
                    self.pick_tolerance,
                    self.geometry_policy,
                )
            })
            .flatten();
        self.set_fillet_branch_preview(target.as_ref())
    }

    /// Activates an explicitly previewed Fillet branch alternative.
    ///
    /// Activation reuses the hover/focus resolver and succeeds only when the
    /// resulting exact target is already the active preview. A direct click,
    /// stale DOM target, proximity spoof or focus race therefore cannot commit
    /// an alternative that the user was not just shown.
    pub fn activate_fillet_action(
        &mut self,
        scene: &EditorScene,
        input: SceneFilletActionInput,
    ) -> Vec<EditorEffect> {
        if self.tool != EditorTool::Select || self.active_pointer_gesture().is_some() {
            return Vec::new();
        }
        let Some(target) = scene.resolve_fillet_action_with_policy(
            input,
            self.pick_tolerance,
            self.geometry_policy,
        ) else {
            return Vec::new();
        };
        if self.fillet_branch_preview != Some(target) {
            return Vec::new();
        }
        self.fillet_branch_preview = None;
        vec![
            EditorEffect::CommitComputedFilletAction { target },
            EditorEffect::FilletBranchPreviewChanged { target: None },
        ]
    }

    /// Clears any active Fillet branch preview without changing feature intent.
    pub fn clear_fillet_branch_preview(&mut self) -> Vec<EditorEffect> {
        self.set_fillet_branch_preview(None)
    }

    /// Drops a preview whose exact stamp/action is no longer present and applicable.
    ///
    /// Presentation adapters call this after constructing a replacement scene;
    /// ordinary pointer/focus transitions also reconcile through their resolver.
    pub fn reconcile_fillet_branch_preview(&mut self, scene: &EditorScene) -> Vec<EditorEffect> {
        let retained = self.fillet_branch_preview.filter(|target| {
            scene.resolve_fillet_action_with_policy(
                SceneFilletActionInput::Accessible(*target),
                self.pick_tolerance,
                self.geometry_policy,
            ) == Some(*target)
        });
        self.set_fillet_branch_preview(retained.as_ref())
    }

    fn set_fillet_branch_preview(
        &mut self,
        target: Option<&SceneFilletActionTarget>,
    ) -> Vec<EditorEffect> {
        let target = target.copied();
        if self.fillet_branch_preview == target {
            return Vec::new();
        }
        self.fillet_branch_preview = target;
        vec![EditorEffect::FilletBranchPreviewChanged { target }]
    }

    /// Returns the persistent item currently under the pointer.
    #[must_use]
    pub const fn hovered(&self) -> Option<SelectionItem> {
        match self.hover_target {
            Some(target) => Some(target.item()),
            None => None,
        }
    }

    /// Returns exact pointer proximity separately from retained reveal context.
    #[must_use]
    pub const fn hover_state(&self) -> EditorHoverState {
        EditorHoverState {
            target: self.hover_target,
            context_owner: match self.hover_context {
                Some(context) => Some(context.owner),
                None => None,
            },
        }
    }

    pub(crate) fn set_authoring_hover_target(
        &mut self,
        item: Option<SelectionItem>,
    ) -> Vec<EditorEffect> {
        self.set_hover_state(item.map(EditorHoverTarget::Geometry), None)
    }

    /// Returns the explicit user-picked parameter for one selected curve span.
    #[must_use]
    pub fn curve_pick_parameter(&self, span: CurveSpan) -> Option<f64> {
        self.curve_pick_parameters
            .iter()
            .find_map(|(candidate, parameter)| (*candidate == span).then_some(*parameter))
    }

    /// Returns the exact visible native-source occurrence responsible for one
    /// selected curve's most recent canvas pick.
    #[must_use]
    pub fn curve_pick_origin(&self, span: CurveSpan) -> Option<SceneCurveOrigin> {
        self.curve_pick_origins
            .iter()
            .find_map(|(candidate, origin)| (*candidate == span).then_some(*origin))
    }

    /// Replaces ordered persistent selection, removing later duplicates.
    pub fn set_selection(&mut self, selection: impl IntoIterator<Item = SelectionItem>) {
        let previous = self.selection.clone();
        self.fillet_branch_preview = None;
        self.selection.clear();
        self.curve_pick_parameters.clear();
        self.curve_pick_origins.clear();
        for item in selection {
            if !self.selection.contains(&item) {
                self.selection.push(item);
            }
        }
        if self.selection != previous {
            self.curve_control_gesture = None;
            // This setter predates effect-returning interaction transitions. Keep
            // its unit API while revoking the cached prediction whose annotation
            // visibility was evaluated against the previous selection. Hosts
            // already rerender after replacing selection.
            let _ = self.invalidate_pointer_context();
        }
    }

    pub(crate) fn revoke_computed_feature_interaction(
        &mut self,
        feature: geosolve_sketch_features::ComputedFeatureId,
    ) {
        if self
            .fillet_branch_preview
            .is_some_and(|target| target.owner.feature == feature)
        {
            self.fillet_branch_preview = None;
        }
        if self
            .computed_fillet_continuation_status
            .as_ref()
            .is_some_and(|status| status.owner.feature == feature)
        {
            self.computed_fillet_continuation_status = None;
        }
        self.selection
            .retain(|item| !item_belongs_to_computed_feature(*item, feature));
        self.curve_pick_parameters
            .retain(|(span, _)| self.selection.contains(&SelectionItem::Curve(*span)));
        self.curve_pick_origins
            .retain(|(span, _)| self.selection.contains(&SelectionItem::Curve(*span)));
        if self
            .hover_target
            .is_some_and(|target| item_belongs_to_computed_feature(target.item(), feature))
        {
            self.hover_target = None;
        }
        if self
            .hover_context
            .is_some_and(|context| item_belongs_to_computed_feature(context.owner, feature))
        {
            self.hover_context = None;
        }
        if self
            .feature_radius_gesture
            .is_some_and(|gesture| gesture.owner.feature == feature)
        {
            self.feature_radius_gesture = None;
        }
        if self
            .feature_contact_gesture
            .is_some_and(|gesture| gesture.owner.feature == feature)
        {
            self.feature_contact_gesture = None;
        }
    }

    /// Applies one toolkit-independent selection click.
    pub fn select_item(&mut self, item: SelectionItem, modifiers: Modifiers) {
        let previous = self.selection.clone();
        self.fillet_branch_preview = None;
        if modifiers.extends_selection() {
            if let Some(index) = self.selection.iter().position(|selected| *selected == item) {
                self.selection.remove(index);
                if let SelectionItem::Curve(span) = item {
                    self.curve_pick_parameters
                        .retain(|(candidate, _)| *candidate != span);
                    self.curve_pick_origins
                        .retain(|(candidate, _)| *candidate != span);
                }
            } else {
                self.selection.push(item);
            }
        } else {
            self.selection.clear();
            self.curve_pick_parameters.clear();
            self.curve_pick_origins.clear();
            self.selection.push(item);
        }
        if self.selection != previous {
            self.curve_control_gesture = None;
            // Preserve the established unit-returning API. Direct callers and
            // pointer-down both rerender through their selection mutation path,
            // so clearing the authoritative state is sufficient and avoids a
            // second public selection/effect surface.
            let _ = self.invalidate_pointer_context();
        }
    }

    fn resolve_select_pointer_target(
        &self,
        scene: &EditorScene,
        position: ScreenPoint,
        problem_items: &[SelectionItem],
    ) -> Option<ResolvedSelectPointerTarget> {
        // This order is the single Select-mode pointer contract. In particular,
        // direct manipulation predicts the same click through visible leaders,
        // while annotations still precede passive geometry and intrinsic datums.
        if !position.is_finite() || !self.pick_tolerance.is_valid() {
            return None;
        }
        if let Some(SceneFilletHit::Radius {
            owner,
            distance_pixels,
        }) = scene.resolve_fillet_hit_with_policy(
            position,
            self.pick_tolerance,
            self.geometry_policy,
        ) {
            return Some(ResolvedSelectPointerTarget::FilletRadius(Hit {
                item: SelectionItem::FeatureCorner(owner),
                distance_pixels,
                curve_parameter: None,
                geometry: None,
            }));
        }
        if let [SelectionItem::Curve(selected)] = self.selection.as_slice()
            && let Some(hit) = scene.curve_control_hit_test_with_policy(
                position,
                self.pick_tolerance,
                self.geometry_policy,
            )
            && hit.owner() == *selected
        {
            return Some(ResolvedSelectPointerTarget::CurveControl(hit));
        }
        if let Some(hit) = scene.draggable_geometry_hit_test_with_policy(
            position,
            self.pick_tolerance,
            self.geometry_policy,
        ) {
            return Some(ResolvedSelectPointerTarget::Geometry(hit));
        }
        let geometry_hit = scene.geometry_hit_test_with_policy(
            position,
            self.pick_tolerance,
            self.geometry_policy,
        );
        let datum_hit = scene.datum_hit_test(position, self.geometry_policy);
        // A passive geometry/datum hit reveals its contextual annotations at
        // this very sample. Use that prospective context for both move and down
        // so the first hover cannot paint the lower owner and then let a newly
        // revealed annotation steal an unchanged click.
        let retained_visibility_context = self.visibility_context();
        let prospective_visibility_context = geometry_hit.or(datum_hit).map(|hit| hit.item);
        let retained_annotation_hit = scene.annotation_occurrence_hit_test(
            position,
            self.pick_tolerance,
            &self.selection,
            retained_visibility_context,
            problem_items,
        );
        let prospective_annotation_hit = (prospective_visibility_context
            != retained_visibility_context)
            .then(|| {
                scene.annotation_occurrence_hit_test(
                    position,
                    self.pick_tolerance,
                    &self.selection,
                    prospective_visibility_context,
                    problem_items,
                )
            })
            .flatten();
        let annotation_hit = retained_annotation_hit
            .into_iter()
            .chain(prospective_annotation_hit)
            .min_by(|first, second| {
                first
                    .1
                    .total_cmp(&second.1)
                    .then_with(|| first.0.item.cmp(&second.0.item))
                    .then_with(|| first.0.marker_index.cmp(&second.0.marker_index))
            });
        if let Some((occurrence, distance_pixels)) = annotation_hit {
            return Some(ResolvedSelectPointerTarget::Annotation {
                occurrence,
                distance_pixels,
            });
        }
        geometry_hit
            .map(ResolvedSelectPointerTarget::Geometry)
            .or_else(|| datum_hit.map(ResolvedSelectPointerTarget::Geometry))
    }

    /// Preflights the exact shared Select resolver for a curve-control-owned press.
    ///
    /// The retained coordinator uses this before forwarding pointer-down so it can
    /// authenticate the accepted scene and complete selected-curve catalog for both
    /// direct controls and stored-point aliases. Ordinary point presses deliberately
    /// remain outside that additional curve-control provenance check.
    pub(crate) fn curve_control_press_target(
        &self,
        scene: &EditorScene,
        position: ScreenPoint,
        problem_items: &[SelectionItem],
    ) -> Option<SceneCurveControlHit> {
        if self.tool != EditorTool::Select || !position.is_finite() {
            return None;
        }
        match self.resolve_select_pointer_target(scene, position, problem_items) {
            Some(ResolvedSelectPointerTarget::CurveControl(hit)) => Some(hit),
            _ => None,
        }
    }

    /// Resolves a pointer press and changes selection immediately.
    pub fn pointer_down(&mut self, scene: &EditorScene, input: PointerInput) -> Vec<EditorEffect> {
        self.pointer_down_with_draft_inference(scene, input, DraftInferenceInput::default())
    }

    /// Resolves a pointer press with explicit host-normalized inference input.
    pub fn pointer_down_with_draft_inference(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
        inference: DraftInferenceInput,
    ) -> Vec<EditorEffect> {
        self.pointer_down_with_draft_authoring(
            scene,
            input,
            DraftAuthoringInput {
                inference,
                regularized: false,
            },
        )
    }

    /// Resolves a pointer press with independent ambient-inference and recipe
    /// regularization intent.
    pub fn pointer_down_with_draft_authoring(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
        authoring: DraftAuthoringInput,
    ) -> Vec<EditorEffect> {
        self.pointer_down_with_problem_items_and_draft_authoring(scene, input, &[], authoring)
    }

    /// Resolves a pointer press while including diagnostically forced annotations.
    pub fn pointer_down_with_problem_items(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
        problem_items: &[SelectionItem],
    ) -> Vec<EditorEffect> {
        self.pointer_down_with_problem_items_and_draft_inference(
            scene,
            input,
            problem_items,
            DraftInferenceInput::default(),
        )
    }

    /// Resolves a pointer press with both diagnostic annotation forcing and
    /// explicit host-normalized drafting inference input.
    pub fn pointer_down_with_problem_items_and_draft_inference(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
        problem_items: &[SelectionItem],
        inference: DraftInferenceInput,
    ) -> Vec<EditorEffect> {
        self.pointer_down_with_problem_items_and_draft_authoring(
            scene,
            input,
            problem_items,
            DraftAuthoringInput {
                inference,
                regularized: false,
            },
        )
    }

    /// Resolves a pointer press with problem-aware annotation visibility and
    /// complete semantic geometry-authoring input.
    pub fn pointer_down_with_problem_items_and_draft_authoring(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
        problem_items: &[SelectionItem],
        authoring: DraftAuthoringInput,
    ) -> Vec<EditorEffect> {
        if !input.position.is_finite() {
            return Vec::new();
        }
        let mut effects = self.clear_fillet_branch_preview();
        if self.tool != EditorTool::Select {
            effects.extend(self.draft_down(scene, input, authoring));
            return effects;
        }
        if self.annotation_gesture.is_some()
            || self.curve_control_gesture.is_some()
            || self.feature_radius_gesture.is_some()
            || self.feature_contact_gesture.is_some()
        {
            return effects;
        }
        let target = self.resolve_select_pointer_target(scene, input.position, problem_items);
        let resolved_input = if matches!(
            target,
            Some(
                ResolvedSelectPointerTarget::FilletRadius(_)
                    | ResolvedSelectPointerTarget::CurveControl(_)
            )
        ) {
            PointerInput {
                modifiers: Modifiers::default(),
                ..input
            }
        } else {
            input
        };
        match target {
            Some(ResolvedSelectPointerTarget::CurveControl(hit)) => {
                effects.extend(self.pointer_down_curve_control(scene, resolved_input, hit));
            }
            target => effects.extend(self.pointer_down_resolved_hit(
                scene,
                resolved_input,
                target.map(ResolvedSelectPointerTarget::hit),
            )),
        }
        if let Some(ResolvedSelectPointerTarget::Annotation { occurrence, .. }) = target
            && scene
                .annotations
                .iter()
                .find(|annotation| annotation.item == occurrence.item)
                .is_some_and(|annotation| {
                    annotation.movable_handle_hit(input.position, occurrence.marker_index)
                })
            && let Some(annotation) = scene
                .annotations
                .iter()
                .find(|annotation| annotation.item == occurrence.item)
            && let Some(automatic) = annotation.automatic_placement(occurrence.marker_index)
        {
            let key = annotation.layout_key(scene.accepted_document.id(), occurrence.marker_index);
            self.annotation_gesture = Some(AnnotationGesture {
                pointer_id: input.pointer_id,
                key,
                accepted_revision: scene.accepted_revision,
                design_identity: scene.design_identity,
                viewport: scene.viewport,
                origin: input.position,
                original: self.annotation_layout.get(key),
                automatic,
                preview: None,
                moved: false,
            });
        }
        effects
    }

    fn pointer_down_curve_control(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
        hit: SceneCurveControlHit,
    ) -> Vec<EditorEffect> {
        if self.tool != EditorTool::Select
            || !input.position.is_finite()
            || self.selection.as_slice() != [SelectionItem::Curve(hit.owner())]
        {
            return Vec::new();
        }
        let Some(control) = scene.curve_controls.iter().find(|candidate| {
            candidate.id == hit.control()
                && candidate.owner == hit.owner()
                && candidate.is_editable()
        }) else {
            return Vec::new();
        };
        match hit {
            SceneCurveControlHit::PointAlias { point, .. } => {
                let effects = self.cancel_point_gesture();
                self.curve_control_gesture = None;
                let Some(point_position) = scene
                    .points
                    .iter()
                    .find(|candidate| {
                        candidate.id == point
                            && model_positions_bit_equal(
                                candidate.model_position,
                                control.model_position,
                            )
                            && candidate.screen_position == control.screen_position
                    })
                    .map(|_| control.model_position)
                else {
                    return effects;
                };
                let pointer_position = scene.viewport.screen_to_model(input.position);
                if let Some(epoch) = self.next_point_gesture_epoch.checked_add(1) {
                    self.next_point_gesture_epoch = epoch;
                    self.point_gesture = Some(PointGesture {
                        epoch,
                        pointer_id: input.pointer_id,
                        point,
                        origin: input.position,
                        model_offset: [
                            point_position[0] - pointer_position[0],
                            point_position[1] - pointer_position[1],
                        ],
                        moved: false,
                        latest_request: None,
                    });
                    self.last_valid_drag_preview = None;
                }
                effects
            }
            SceneCurveControlHit::Direct { .. } => {
                let mut effects = self.cancel_point_gesture();
                effects.extend(self.cancel_curve_control_gesture());
                let pointer_position = scene.viewport.screen_to_model(input.position);
                self.curve_control_gesture = Some(CurveControlGesture {
                    pointer_id: input.pointer_id,
                    control: control.id,
                    owner: control.owner,
                    expected: scene.design_identity,
                    accepted_revision: scene.accepted_revision,
                    viewport: scene.viewport,
                    origin: input.position,
                    origin_model: pointer_position,
                    model_position: control.model_position,
                    model_offset: [
                        control.model_position[0] - pointer_position[0],
                        control.model_position[1] - pointer_position[1],
                    ],
                    rail: control.rail,
                    moved: false,
                    last_sampled_position: Some(control.model_position),
                    latest_request: None,
                    last_valid_request: None,
                });
                self.last_valid_drag_preview = None;
                effects
            }
        }
    }

    /// Starts a computed-Fillet gesture for one explicitly painted preview arc.
    ///
    /// The coordinator validates current preview ownership and scene provenance
    /// before calling this path. This final editor-side check still requires the
    /// pointer to hit that exact owner's radius affordance or, for an older
    /// scene without affordances, its computed curve. A presentation target is
    /// an intent hint rather than a geometry oracle.
    pub(crate) fn pointer_down_feature_radius(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
        owner: geosolve_sketch_features::ComputedCornerRef,
        tolerance: PickTolerance,
    ) -> Option<Vec<EditorEffect>> {
        if self.feature_radius_gesture.is_some()
            || self.feature_contact_gesture.is_some()
            || !input.position.is_finite()
            || self.tool != EditorTool::Select
            || !tolerance.is_valid()
        {
            return None;
        }
        let direct_input = PointerInput {
            modifiers: Modifiers::default(),
            ..input
        };
        let hit = self.resolve_feature_radius_hit(scene, input.position, owner, tolerance)?;
        let mut effects = self.pointer_down_resolved_hit(scene, direct_input, Some(hit));
        let mut combined = self.clear_fillet_branch_preview();
        combined.append(&mut effects);
        Some(combined)
    }

    pub(crate) fn feature_radius_hover_item(
        &self,
        scene: &EditorScene,
        position: ScreenPoint,
        owner: geosolve_sketch_features::ComputedCornerRef,
        tolerance: PickTolerance,
    ) -> Option<SelectionItem> {
        if self.feature_radius_gesture.is_some()
            || self.feature_contact_gesture.is_some()
            || self.tool != EditorTool::Select
        {
            return None;
        }
        self.resolve_feature_radius_hit(scene, position, owner, tolerance)
            .map(|hit| hit.item)
    }

    fn resolve_feature_radius_hit(
        &self,
        scene: &EditorScene,
        position: ScreenPoint,
        owner: geosolve_sketch_features::ComputedCornerRef,
        tolerance: PickTolerance,
    ) -> Option<Hit> {
        if !position.is_finite() || !tolerance.is_valid() {
            return None;
        }
        match scene.resolve_fillet_hit_with_policy(position, tolerance, self.geometry_policy) {
            Some(SceneFilletHit::Radius {
                owner: resolved,
                distance_pixels,
            }) if resolved == owner => Some(Hit {
                item: SelectionItem::FeatureCorner(owner),
                distance_pixels,
                curve_parameter: None,
                geometry: None,
            }),
            Some(SceneFilletHit::Radius { .. }) => None,
            Some(SceneFilletHit::Native(_)) | None => scene
                .computed_curves
                .iter()
                .find(|curve| curve.owner == owner && curve.is_pickable(self.geometry_policy))
                .and_then(|curve| computed_curve_hit(curve, position, tolerance.curve_pixels)),
        }
    }

    #[cfg(test)]
    fn pointer_down_feature_contact_handle(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
        handle: SceneFilletContactHandle,
    ) -> Vec<EditorEffect> {
        let Some(expected) = scene.computed_input else {
            return Vec::new();
        };
        let mut effects = self.cancel_point_gesture();
        let before = self.selection.clone();
        self.select_item(
            SelectionItem::FeatureCorner(handle.owner),
            Modifiers::default(),
        );
        effects.extend(
            (before != self.selection)
                .then(|| EditorEffect::SelectionChanged(self.selection.clone())),
        );
        self.feature_radius_gesture = None;
        self.computed_fillet_continuation_status = None;
        self.feature_contact_gesture = Some(FeatureContactGesture {
            pointer_id: input.pointer_id,
            owner: handle.owner,
            parent: handle.parent,
            source: handle.source,
            expected,
            viewport: scene.viewport,
            origin: input.position,
            moved: false,
            origin_parameter: handle.parameter,
            last_sampled_parameter: Some(handle.parameter),
            last_sampled_position: Some(input.position),
            last_requested_parameter: None,
            last_accepted_parameter: None,
        });
        self.last_valid_drag_preview = None;
        effects
    }

    fn pointer_down_resolved_hit(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
        hit: Option<Hit>,
    ) -> Vec<EditorEffect> {
        let mut effects = Vec::new();
        if self
            .point_gesture
            .is_some_and(|gesture| gesture.pointer_id != input.pointer_id)
        {
            effects.extend(self.cancel_point_gesture());
        }
        if self
            .feature_radius_gesture
            .is_some_and(|gesture| gesture.pointer_id != input.pointer_id)
        {
            effects.extend(self.cancel_feature_radius_gesture());
        }
        let before = self.selection.clone();
        if let Some(hit) = hit {
            self.select_item(hit.item, input.modifiers);
            if let (SelectionItem::Curve(span), Some(parameter)) = (hit.item, hit.curve_parameter) {
                self.curve_pick_parameters
                    .retain(|(candidate, _)| *candidate != span);
                self.curve_pick_origins
                    .retain(|(candidate, _)| *candidate != span);
                if self.selection.contains(&hit.item) {
                    self.curve_pick_parameters.push((span, parameter));
                    if let Some(SceneGeometryHit::NativeCurve { origin, .. }) = hit.geometry {
                        self.curve_pick_origins.push((span, origin));
                    }
                }
            }
            if let Some((owner, radius, rail)) = scene.feature_radius_handle(hit.item)
                && self.selection.contains(&hit.item)
                && !self
                    .selection
                    .iter()
                    .any(|item| matches!(item, SelectionItem::Datum(_)))
                && let Some(expected) = scene.computed_input
            {
                let pointer = scene.viewport.screen_to_model(input.position);
                self.feature_radius_gesture = Some(FeatureRadiusGesture {
                    pointer_id: input.pointer_id,
                    owner,
                    expected,
                    origin: input.position,
                    origin_model: pointer,
                    model_derivative: rail.model_derivative,
                    viewport: scene.viewport,
                    moved: false,
                    origin_radius: radius,
                    last_sampled_radius: Some(radius),
                    last_sampled_position: Some(input.position),
                    last_requested_radius: None,
                    last_accepted_radius: None,
                });
                self.computed_fillet_continuation_status = None;
                self.point_gesture = None;
                self.last_valid_drag_preview = None;
            }
            if let Some(point) = scene.drag_handle_point(hit.item)
                && self.selection.contains(&hit.item)
                && !self
                    .selection
                    .iter()
                    .any(|item| matches!(item, SelectionItem::Datum(_)))
                && let Some(point_position) = scene
                    .points
                    .iter()
                    .find(|candidate| candidate.id == point)
                    .map(|candidate| candidate.model_position)
            {
                let pointer_position = scene.viewport.screen_to_model(input.position);
                if let Some(epoch) = self.next_point_gesture_epoch.checked_add(1) {
                    self.next_point_gesture_epoch = epoch;
                    self.point_gesture = Some(PointGesture {
                        epoch,
                        pointer_id: input.pointer_id,
                        point,
                        origin: input.position,
                        model_offset: [
                            point_position[0] - pointer_position[0],
                            point_position[1] - pointer_position[1],
                        ],
                        moved: false,
                        latest_request: None,
                    });
                    self.last_valid_drag_preview = None;
                }
            }
        } else if !input.modifiers.extends_selection() {
            self.selection.clear();
            self.curve_pick_parameters.clear();
            self.curve_pick_origins.clear();
        }
        effects.extend(
            (before != self.selection)
                .then(|| EditorEffect::SelectionChanged(self.selection.clone())),
        );
        effects
    }

    /// Advances an active point gesture and emits projected-preview work only after
    /// the configured screen-space movement threshold.
    pub fn pointer_move(&mut self, scene: &EditorScene, input: PointerInput) -> Vec<EditorEffect> {
        self.pointer_move_with_problem_items_and_draft_inference(
            scene,
            input,
            &[],
            DraftInferenceInput::default(),
        )
    }

    /// Advances a gesture with explicit host-normalized drafting inference input.
    pub fn pointer_move_with_draft_inference(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
        inference: DraftInferenceInput,
    ) -> Vec<EditorEffect> {
        self.pointer_move_with_draft_authoring(
            scene,
            input,
            DraftAuthoringInput {
                inference,
                regularized: false,
            },
        )
    }

    /// Advances a pointer sample with independent ambient-inference and recipe
    /// regularization intent.
    pub fn pointer_move_with_draft_authoring(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
        authoring: DraftAuthoringInput,
    ) -> Vec<EditorEffect> {
        self.pointer_move_with_problem_items_and_draft_authoring(scene, input, &[], authoring)
    }

    /// Advances a pointer sample while including diagnostically forced annotations.
    pub fn pointer_move_with_problem_items(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
        problem_items: &[SelectionItem],
    ) -> Vec<EditorEffect> {
        self.pointer_move_with_problem_items_and_draft_inference(
            scene,
            input,
            problem_items,
            DraftInferenceInput::default(),
        )
    }

    /// Advances a pointer sample with both diagnostic annotation forcing and
    /// explicit host-normalized drafting inference input.
    pub fn pointer_move_with_problem_items_and_draft_inference(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
        problem_items: &[SelectionItem],
        inference: DraftInferenceInput,
    ) -> Vec<EditorEffect> {
        self.pointer_move_with_problem_items_and_draft_authoring(
            scene,
            input,
            problem_items,
            DraftAuthoringInput {
                inference,
                regularized: false,
            },
        )
    }

    /// Advances a pointer sample with problem-aware annotation visibility and
    /// complete semantic geometry-authoring input.
    pub fn pointer_move_with_problem_items_and_draft_authoring(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
        problem_items: &[SelectionItem],
        authoring: DraftAuthoringInput,
    ) -> Vec<EditorEffect> {
        let mut effects = self.clear_fillet_branch_preview();
        if self.tool != EditorTool::Select {
            effects.extend(self.draft_move(scene, input, authoring));
            return effects;
        }
        if self.feature_contact_gesture.is_some() {
            effects.extend(self.move_feature_contact_gesture(scene, input));
            return effects;
        }
        if self.feature_radius_gesture.is_some() {
            effects.extend(self.move_feature_radius_gesture(scene, input));
            return effects;
        }
        if self.curve_control_gesture.is_some() {
            effects.extend(self.move_curve_control_gesture(scene, input));
            return effects;
        }
        if self.point_gesture.is_none() {
            if self.annotation_gesture.is_some() {
                self.move_annotation_gesture(scene, input);
                return effects;
            }
            effects.extend(self.move_hover(scene, input, problem_items));
            return effects;
        }
        effects.extend(self.move_point_gesture(scene, input));
        effects
    }

    fn move_annotation_gesture(&mut self, scene: &EditorScene, input: PointerInput) {
        let Some(mut gesture) = self.annotation_gesture.clone() else {
            return;
        };
        if gesture.pointer_id != input.pointer_id || !input.position.is_finite() {
            return;
        }
        if scene.accepted_revision != gesture.accepted_revision
            || scene.design_identity != gesture.design_identity
            || scene.viewport != gesture.viewport
            || scene.accepted_document.id() != gesture.key.document
        {
            self.annotation_gesture = None;
            return;
        }
        gesture.moved |= gesture.origin.distance(input.position) >= self.drag_threshold_pixels;
        if !gesture.moved {
            self.annotation_gesture = Some(gesture);
            return;
        }
        let Some(annotation) = scene
            .annotations
            .iter()
            .find(|annotation| annotation.item == gesture.key.item)
        else {
            return;
        };
        let base = gesture.original.unwrap_or(gesture.automatic);
        let delta = [
            input.position.x - gesture.origin.x,
            input.position.y - gesture.origin.y,
        ];
        let placement = match (&annotation.geometry, base) {
            (
                SceneAnnotationGeometry::LinearDimension {
                    measured_first,
                    measured_second,
                    ..
                },
                AnnotationPlacement::Linear {
                    perpendicular_pixels,
                },
            ) => {
                let Some(direction) = screen_unit(
                    measured_second.x - measured_first.x,
                    measured_second.y - measured_first.y,
                ) else {
                    return;
                };
                AnnotationPlacement::Linear {
                    perpendicular_pixels: perpendicular_pixels
                        + delta[0].mul_add(-direction[1], delta[1] * direction[0]),
                }
            }
            (
                SceneAnnotationGeometry::RadialDimension {
                    center,
                    edge,
                    full_circle,
                    ..
                },
                AnnotationPlacement::Radial { .. },
            ) => {
                let radial = [input.position.x - center.x, input.position.y - center.y];
                let radius = center.distance(*edge);
                let (direction_radians, distance) = if *full_circle {
                    let distance = radial[0].hypot(radial[1]);
                    if distance <= f64::EPSILON {
                        return;
                    }
                    (radial[1].atan2(radial[0]), distance)
                } else {
                    let Some(direction) = screen_unit(edge.x - center.x, edge.y - center.y) else {
                        return;
                    };
                    (
                        direction[1].atan2(direction[0]),
                        radial[0].mul_add(direction[0], radial[1] * direction[1]),
                    )
                };
                AnnotationPlacement::Radial {
                    direction_radians,
                    clearance_pixels: distance - radius,
                }
            }
            (
                SceneAnnotationGeometry::AngularDimension { vertex, .. },
                AnnotationPlacement::Angular { .. },
            ) => AnnotationPlacement::Angular {
                radius_pixels: (vertex.distance(input.position) - 18.0).max(12.0),
            },
            (_, AnnotationPlacement::Free { offset_pixels }) => AnnotationPlacement::Free {
                offset_pixels: [offset_pixels[0] + delta[0], offset_pixels[1] + delta[1]],
            },
            _ => AnnotationPlacement::Free {
                offset_pixels: delta,
            },
        };
        gesture.preview = Some(placement);
        self.annotation_gesture = Some(gesture);
    }

    fn move_curve_control_gesture(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
    ) -> Vec<EditorEffect> {
        let Some(mut gesture) = self.curve_control_gesture else {
            return Vec::new();
        };
        if gesture.pointer_id != input.pointer_id || !input.position.is_finite() {
            return Vec::new();
        }
        if !scene.accepts_curve_control_gesture(
            gesture.accepted_revision,
            gesture.expected,
            gesture.viewport,
            gesture.control,
            gesture.owner,
            gesture.last_valid_request,
        ) {
            return self.cancel_curve_control_gesture();
        }
        gesture.moved |= gesture.origin.distance(input.position) >= self.drag_threshold_pixels;
        if !gesture.moved {
            self.curve_control_gesture = Some(gesture);
            return Vec::new();
        }
        let Some(model_position) = Self::curve_control_sample(scene, &gesture, input.position)
        else {
            self.curve_control_gesture = Some(gesture);
            return Vec::new();
        };
        if gesture
            .last_sampled_position
            .is_some_and(|sampled| model_positions_bit_equal(sampled, model_position))
        {
            self.curve_control_gesture = Some(gesture);
            return Vec::new();
        }
        let request_id = self.next_projection_request;
        let Some(next_request) = request_id.checked_add(1) else {
            self.curve_control_gesture = Some(gesture);
            return Vec::new();
        };
        self.next_projection_request = next_request;
        gesture.last_sampled_position = Some(model_position);
        gesture.latest_request = Some(request_id);
        self.curve_control_gesture = Some(gesture);
        vec![EditorEffect::RequestCurveControlPreview {
            pointer_id: gesture.pointer_id,
            request_id,
            expected: gesture.expected,
            control: gesture.control,
            model_position,
        }]
    }

    fn curve_control_sample(
        scene: &EditorScene,
        gesture: &CurveControlGesture,
        position: ScreenPoint,
    ) -> Option<[f64; 2]> {
        if !position.is_finite() || scene.viewport != gesture.viewport {
            return None;
        }
        let pointer = scene.viewport.screen_to_model(position);
        let model_position = if let Some(rail) = gesture.rail {
            let delta = [
                pointer[0] - gesture.origin_model[0],
                pointer[1] - gesture.origin_model[1],
            ];
            let along =
                delta[0].mul_add(rail.model_direction[0], delta[1] * rail.model_direction[1]);
            let target = [
                along.mul_add(rail.model_direction[0], gesture.model_position[0]),
                along.mul_add(rail.model_direction[1], gesture.model_position[1]),
            ];
            let signed = (target[0] - rail.model_zero[0]).mul_add(
                rail.model_direction[0],
                (target[1] - rail.model_zero[1]) * rail.model_direction[1],
            );
            (signed.is_finite() && signed > 0.0).then_some(target)?
        } else {
            [
                pointer[0] + gesture.model_offset[0],
                pointer[1] + gesture.model_offset[1],
            ]
        };
        model_position
            .into_iter()
            .all(f64::is_finite)
            .then_some(model_position)
    }

    fn move_feature_contact_gesture(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
    ) -> Vec<EditorEffect> {
        let Some(mut gesture) = self.feature_contact_gesture else {
            return Vec::new();
        };
        if gesture.pointer_id != input.pointer_id || !input.position.is_finite() {
            return Vec::new();
        }
        gesture.moved |= gesture.origin.distance(input.position) >= self.drag_threshold_pixels;
        if !gesture.moved {
            self.feature_contact_gesture = Some(gesture);
            return Vec::new();
        }
        let Some(parameter) = Self::feature_contact_sample(scene, &gesture, input.position) else {
            gesture.last_sampled_parameter = None;
            gesture.last_sampled_position = None;
            gesture.last_requested_parameter = None;
            gesture.last_accepted_parameter = None;
            self.feature_contact_gesture = Some(gesture);
            return Vec::new();
        };
        if gesture
            .last_sampled_parameter
            .is_some_and(|sampled| sampled.to_bits() == parameter.to_bits())
        {
            gesture.last_sampled_position = Some(input.position);
            self.feature_contact_gesture = Some(gesture);
            return Vec::new();
        }
        gesture.last_sampled_parameter = Some(parameter);
        gesture.last_sampled_position = Some(input.position);
        gesture.last_requested_parameter = Some(parameter);
        gesture.last_accepted_parameter = None;
        self.computed_fillet_continuation_status = None;
        self.feature_contact_gesture = Some(gesture);
        vec![EditorEffect::PreviewComputedFeatureContact {
            expected: gesture.expected,
            owner: gesture.owner,
            parent: gesture.parent,
            source: gesture.source,
            parameter,
        }]
    }

    fn move_feature_radius_gesture(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
    ) -> Vec<EditorEffect> {
        let Some(mut gesture) = self.feature_radius_gesture else {
            return Vec::new();
        };
        if gesture.pointer_id != input.pointer_id || !input.position.is_finite() {
            return Vec::new();
        }
        gesture.moved |= gesture.origin.distance(input.position) >= self.drag_threshold_pixels;
        if !gesture.moved {
            self.feature_radius_gesture = Some(gesture);
            return Vec::new();
        }
        let Some(radius) = Self::feature_radius_sample(scene, &gesture, input.position) else {
            gesture.last_sampled_radius = None;
            gesture.last_sampled_position = None;
            gesture.last_requested_radius = None;
            gesture.last_accepted_radius = None;
            self.feature_radius_gesture = Some(gesture);
            return Vec::new();
        };
        if gesture
            .last_sampled_radius
            .is_some_and(|sampled| sampled.to_bits() == radius.to_bits())
        {
            gesture.last_sampled_position = Some(input.position);
            self.feature_radius_gesture = Some(gesture);
            return Vec::new();
        }
        gesture.last_sampled_radius = Some(radius);
        gesture.last_sampled_position = Some(input.position);
        gesture.last_requested_radius = Some(radius);
        gesture.last_accepted_radius = None;
        self.computed_fillet_continuation_status = None;
        self.feature_radius_gesture = Some(gesture);
        vec![EditorEffect::PreviewComputedFeatureRadius {
            expected: gesture.expected,
            feature: gesture.owner.feature,
            radius,
        }]
    }

    fn feature_contact_sample(
        scene: &EditorScene,
        gesture: &FeatureContactGesture,
        position: ScreenPoint,
    ) -> Option<f64> {
        (position.is_finite()
            && scene.viewport == gesture.viewport
            && scene.accepts_fillet_gesture(&gesture.expected))
        .then(|| scene.project_to_native_source(gesture.source, position))
        .flatten()
        .map(|(parameter, _)| parameter)
        .filter(|parameter| parameter.is_finite())
    }

    fn feature_radius_sample(
        scene: &EditorScene,
        gesture: &FeatureRadiusGesture,
        position: ScreenPoint,
    ) -> Option<f64> {
        if !position.is_finite()
            || scene.viewport != gesture.viewport
            || !scene.accepts_fillet_gesture(&gesture.expected)
        {
            return None;
        }
        let position = scene.viewport.screen_to_model(position);
        let pointer_delta = [
            position[0] - gesture.origin_model[0],
            position[1] - gesture.origin_model[1],
        ];
        let derivative_norm_squared = gesture.model_derivative[0].mul_add(
            gesture.model_derivative[0],
            gesture.model_derivative[1].powi(2),
        );
        if !derivative_norm_squared.is_finite() || derivative_norm_squared <= 0.0 {
            return None;
        }
        let radius_delta = pointer_delta[0].mul_add(
            gesture.model_derivative[0],
            pointer_delta[1] * gesture.model_derivative[1],
        ) / derivative_norm_squared;
        let radius = gesture.origin_radius + radius_delta;
        (radius.is_finite() && radius > 0.0).then_some(radius)
    }

    fn move_hover(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
        problem_items: &[SelectionItem],
    ) -> Vec<EditorEffect> {
        if !input.position.is_finite() {
            return Vec::new();
        }
        let resolved = self.resolve_select_pointer_target(scene, input.position, problem_items);
        let (target, context) = match resolved {
            Some(
                target @ (ResolvedSelectPointerTarget::FilletRadius(_)
                | ResolvedSelectPointerTarget::CurveControl(_)
                | ResolvedSelectPointerTarget::Geometry(_)),
            ) => {
                let item = target.hit().item;
                (
                    Some(target.hover_target()),
                    Some(AnnotationHoverContext {
                        owner: item,
                        origin: input.position,
                    }),
                )
            }
            Some(target @ ResolvedSelectPointerTarget::Annotation { occurrence, .. }) => {
                let context = self.hover_context.filter(|context| {
                    scene.annotations.iter().any(|annotation| {
                        annotation.item == occurrence.item
                            && annotation.operands.contains(&context.owner)
                    })
                });
                (Some(target.hover_target()), context)
            }
            None => {
                let context = self.hover_context.filter(|context| {
                    scene.contextual_annotation_transit(
                        input.position,
                        self.pick_tolerance,
                        &self.selection,
                        context.owner,
                        context.origin,
                        problem_items,
                    )
                });
                (None, context)
            }
        };
        self.set_hover_state(target, context)
    }

    fn move_point_gesture(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
    ) -> Vec<EditorEffect> {
        let Some(mut gesture) = self.point_gesture else {
            return Vec::new();
        };
        if gesture.pointer_id != input.pointer_id || !input.position.is_finite() {
            return Vec::new();
        }
        gesture.moved |= gesture.origin.distance(input.position) >= self.drag_threshold_pixels;
        self.point_gesture = Some(gesture);
        if !gesture.moved {
            return Vec::new();
        }
        let request_id = self.next_projection_request;
        let Some(next_request) = request_id.checked_add(1) else {
            return Vec::new();
        };
        self.next_projection_request = next_request;
        gesture.latest_request = Some(request_id);
        self.point_gesture = Some(gesture);
        let pointer_position = scene.viewport.screen_to_model(input.position);
        vec![EditorEffect::RequestProjectedPointMove {
            pointer_id: input.pointer_id,
            request_id,
            point: gesture.point,
            model_position: [
                pointer_position[0] + gesture.model_offset[0],
                pointer_position[1] + gesture.model_offset[1],
            ],
        }]
    }

    /// Clears pointer hover when a presentation surface is left.
    pub fn pointer_leave(&mut self) -> Vec<EditorEffect> {
        let mut effects = self.clear_fillet_branch_preview();
        effects.extend(self.invalidate_pointer_context());
        if self.tool != EditorTool::Select {
            effects.extend(self.invalidate_draft_inference());
        }
        effects
    }

    /// Clears pointer proximity and retained annotation-navigation context.
    ///
    /// Presentation adapters call this when the accepted scene, viewport, or
    /// another pointer-owning interaction surface is remapped without an
    /// ordinary canvas pointer-leave sample.
    fn invalidate_pointer_context(&mut self) -> Vec<EditorEffect> {
        self.set_hover_state(None, None)
    }

    const fn visibility_context(&self) -> Option<SelectionItem> {
        match self.hover_context {
            Some(context) => Some(context.owner),
            None => match self.hover_target {
                Some(target) => Some(target.item()),
                None => None,
            },
        }
    }

    fn set_hover_state(
        &mut self,
        target: Option<EditorHoverTarget>,
        context: Option<AnnotationHoverContext>,
    ) -> Vec<EditorEffect> {
        let before = self.hover_state();
        self.hover_target = target;
        self.hover_context = context;
        let after = self.hover_state();
        if before == after {
            return Vec::new();
        }
        vec![EditorEffect::HoverChanged(after)]
    }

    /// Records that the exact requested Fillet-radius sample produced a
    /// complete current preview.
    ///
    /// Rejected, stale, or out-of-order samples leave the prior accepted sample
    /// untouched. Pointer release can therefore publish only geometry that a
    /// coordinator independently accepted.
    pub(crate) fn accept_computed_feature_radius_preview(
        &mut self,
        expected: &geosolve_sketch_features::ComputedFeatureEvaluationInput,
        feature: geosolve_sketch_features::ComputedFeatureId,
        radius: f64,
    ) -> bool {
        let Some(mut gesture) = self.feature_radius_gesture else {
            return false;
        };
        if gesture.expected != *expected
            || gesture.owner.feature != feature
            || gesture
                .last_requested_radius
                .is_none_or(|requested| requested.to_bits() != radius.to_bits())
        {
            return false;
        }
        gesture.last_requested_radius = None;
        gesture.last_accepted_radius = Some(radius);
        self.feature_radius_gesture = Some(gesture);
        self.computed_fillet_continuation_status = None;
        true
    }

    /// Records a typed failure for the exact latest radius sample while retaining
    /// the coordinator's previous last-current preview.
    ///
    /// Stale, foreign and out-of-order rejection acknowledgements are ignored.
    pub(crate) fn reject_computed_feature_radius_preview(
        &mut self,
        expected: &geosolve_sketch_features::ComputedFeatureEvaluationInput,
        feature: geosolve_sketch_features::ComputedFeatureId,
        radius: f64,
        limit: ComputedFilletContinuationLimit,
    ) -> bool {
        let Some(mut gesture) = self.feature_radius_gesture else {
            return false;
        };
        if gesture.expected != *expected
            || gesture.owner.feature != feature
            || gesture
                .last_requested_radius
                .is_none_or(|requested| requested.to_bits() != radius.to_bits())
            || !radius.is_finite()
            || radius <= 0.0
            || !limit.is_valid()
        {
            return false;
        }
        gesture.last_sampled_radius = None;
        gesture.last_sampled_position = None;
        gesture.last_requested_radius = None;
        gesture.last_accepted_radius = None;
        self.feature_radius_gesture = Some(gesture);
        self.computed_fillet_continuation_status = Some(ComputedFilletContinuationStatus {
            expected: *expected,
            owner: gesture.owner,
            sample: ComputedFilletInteractionSample::Radius(radius),
            limit,
        });
        true
    }

    /// Records that the exact requested contact sample produced a complete
    /// current Fillet preview.
    ///
    /// Stale, foreign and out-of-order acknowledgements are state-neutral.
    pub(crate) fn accept_computed_feature_contact_preview(
        &mut self,
        expected: &geosolve_sketch_features::ComputedFeatureEvaluationInput,
        owner: geosolve_sketch_features::ComputedCornerRef,
        parent: ComputedFilletParentIndex,
        source: NativeCurveSpanSource,
        parameter: f64,
    ) -> bool {
        let Some(mut gesture) = self.feature_contact_gesture else {
            return false;
        };
        if gesture.expected != *expected
            || gesture.owner != owner
            || gesture.parent != parent
            || gesture.source != source
            || gesture
                .last_requested_parameter
                .is_none_or(|requested| requested.to_bits() != parameter.to_bits())
        {
            return false;
        }
        gesture.last_requested_parameter = None;
        gesture.last_accepted_parameter = Some(parameter);
        self.feature_contact_gesture = Some(gesture);
        self.computed_fillet_continuation_status = None;
        true
    }

    /// Records a typed failure for the exact latest named-parent contact sample
    /// while retaining the coordinator's previous last-current preview.
    pub(crate) fn reject_computed_feature_contact_preview(
        &mut self,
        expected: &geosolve_sketch_features::ComputedFeatureEvaluationInput,
        owner: geosolve_sketch_features::ComputedCornerRef,
        parent: ComputedFilletParentIndex,
        source: NativeCurveSpanSource,
        parameter: f64,
        limit: ComputedFilletContinuationLimit,
    ) -> bool {
        let Some(mut gesture) = self.feature_contact_gesture else {
            return false;
        };
        if gesture.expected != *expected
            || gesture.owner != owner
            || gesture.parent != parent
            || gesture.source != source
            || gesture
                .last_requested_parameter
                .is_none_or(|requested| requested.to_bits() != parameter.to_bits())
            || !parameter.is_finite()
            || !limit.is_valid()
        {
            return false;
        }
        gesture.last_sampled_parameter = None;
        gesture.last_sampled_position = None;
        gesture.last_requested_parameter = None;
        gesture.last_accepted_parameter = None;
        self.feature_contact_gesture = Some(gesture);
        self.computed_fillet_continuation_status = Some(ComputedFilletContinuationStatus {
            expected: *expected,
            owner,
            sample: ComputedFilletInteractionSample::Contact {
                parent,
                source,
                parameter,
            },
            limit,
        });
        true
    }

    /// Completes an active point gesture. A click emits no geometry edit.
    #[allow(
        clippy::too_many_lines,
        reason = "one terminal dispatch keeps all captured pointer gesture outcomes mutually exclusive"
    )]
    pub fn pointer_up(
        &mut self,
        scene: &EditorScene,
        expected: SketchDesignIdentity,
        input: PointerInput,
    ) -> Vec<EditorEffect> {
        if self.tool != EditorTool::Select {
            return Vec::new();
        }
        if self
            .annotation_gesture
            .as_ref()
            .is_some_and(|gesture| gesture.pointer_id == input.pointer_id)
        {
            let Some(gesture) = self.annotation_gesture.take() else {
                return Vec::new();
            };
            let current = input.position.is_finite()
                && expected == gesture.design_identity
                && scene.accepted_revision == gesture.accepted_revision
                && scene.design_identity == gesture.design_identity
                && scene.viewport == gesture.viewport
                && scene.accepted_document.id() == gesture.key.document;
            if current
                && gesture.moved
                && let Some(placement) = gesture.preview
            {
                self.annotation_layout.insert(gesture.key, placement);
            }
            return Vec::new();
        }
        if let Some(gesture) = self.curve_control_gesture {
            if gesture.pointer_id != input.pointer_id || !input.position.is_finite() {
                return Vec::new();
            }
            self.curve_control_gesture = None;
            let current = expected == gesture.expected
                && scene.accepts_curve_control_gesture(
                    gesture.accepted_revision,
                    gesture.expected,
                    gesture.viewport,
                    gesture.control,
                    gesture.owner,
                    gesture.last_valid_request,
                );
            return if !gesture.moved {
                Vec::new()
            } else if current {
                gesture.last_valid_request.map_or_else(
                    || vec![EditorEffect::ClearCurveControlPreview],
                    |(request_id, _)| {
                        vec![EditorEffect::CommitCurveControl {
                            expected: gesture.expected,
                            pointer_id: gesture.pointer_id,
                            request_id,
                            control: gesture.control,
                        }]
                    },
                )
            } else {
                vec![EditorEffect::ClearCurveControlPreview]
            };
        }
        if let Some(gesture) = self.feature_contact_gesture {
            if gesture.pointer_id != input.pointer_id || !input.position.is_finite() {
                return Vec::new();
            }
            let current_parameter = (scene.viewport == gesture.viewport
                && scene.accepts_fillet_gesture(&gesture.expected)
                && gesture.last_sampled_position == Some(input.position))
            .then_some(gesture.last_accepted_parameter)
            .flatten();
            self.feature_contact_gesture = None;
            self.computed_fillet_continuation_status = None;
            return if !gesture.moved {
                Vec::new()
            } else if let Some(parameter) = current_parameter {
                vec![
                    EditorEffect::CommitComputedFeatureContact {
                        expected: gesture.expected,
                        owner: gesture.owner,
                        parent: gesture.parent,
                        source: gesture.source,
                        parameter,
                    },
                    EditorEffect::ClearComputedFeatureContactPreview,
                ]
            } else {
                vec![EditorEffect::ClearComputedFeatureContactPreview]
            };
        }
        if let Some(gesture) = self.feature_radius_gesture {
            if gesture.pointer_id != input.pointer_id || !input.position.is_finite() {
                return Vec::new();
            }
            let current_radius = (scene.viewport == gesture.viewport
                && scene.accepts_fillet_gesture(&gesture.expected)
                && gesture.last_sampled_position == Some(input.position))
            .then_some(gesture.last_accepted_radius)
            .flatten();
            self.feature_radius_gesture = None;
            self.computed_fillet_continuation_status = None;
            return if !gesture.moved {
                Vec::new()
            } else if let Some(radius) = current_radius {
                vec![
                    EditorEffect::CommitComputedFeatureRadius {
                        expected: gesture.expected,
                        feature: gesture.owner.feature,
                        radius,
                    },
                    EditorEffect::ClearComputedFeaturePreview,
                ]
            } else {
                vec![EditorEffect::ClearComputedFeaturePreview]
            };
        }
        let Some(gesture) = self.point_gesture else {
            return Vec::new();
        };
        if gesture.pointer_id != input.pointer_id || !input.position.is_finite() {
            return Vec::new();
        }
        self.point_gesture = None;
        if gesture.moved {
            let preview =
                self.last_valid_drag_preview
                    .take()
                    .filter(|(_, epoch, pointer, point, _)| {
                        *epoch == gesture.epoch
                            && *pointer == input.pointer_id
                            && *point == gesture.point
                    });
            preview.map_or_else(
                || vec![EditorEffect::ClearPointPreview],
                |(_, _, _, _, position)| {
                    vec![EditorEffect::CommitPointMove {
                        expected,
                        point: gesture.point,
                        model_position: position,
                    }]
                },
            )
        } else {
            Vec::new()
        }
    }

    /// Cancels an active point gesture without a document edit.
    pub fn cancel(&mut self) -> Vec<EditorEffect> {
        let mut effects = self.cancel_draft();
        effects.extend(self.cancel_point_gesture());
        effects.extend(self.cancel_curve_control_gesture());
        effects.extend(self.cancel_annotation_gesture());
        effects.extend(self.cancel_feature_radius_gesture());
        effects.extend(self.cancel_feature_contact_gesture());
        effects.extend(self.clear_fillet_branch_preview());
        effects.extend(self.invalidate_pointer_context());
        effects
    }

    fn cancel_annotation_gesture(&mut self) -> Vec<EditorEffect> {
        self.annotation_gesture = None;
        Vec::new()
    }

    fn cancel_curve_control_gesture(&mut self) -> Vec<EditorEffect> {
        self.curve_control_gesture
            .take()
            .filter(|gesture| gesture.moved || gesture.last_valid_request.is_some())
            .map(|_| EditorEffect::ClearCurveControlPreview)
            .into_iter()
            .collect()
    }

    fn cancel_feature_radius_gesture(&mut self) -> Vec<EditorEffect> {
        let gesture = self.feature_radius_gesture.take();
        if gesture.is_some() {
            self.computed_fillet_continuation_status = None;
        }
        gesture.map_or_else(Vec::new, |gesture| {
            vec![EditorEffect::RestoreComputedFeatureRadius {
                expected: gesture.expected,
                feature: gesture.owner.feature,
                radius: gesture.origin_radius,
            }]
        })
    }

    fn cancel_feature_contact_gesture(&mut self) -> Vec<EditorEffect> {
        let gesture = self.feature_contact_gesture.take();
        if gesture.is_some() {
            self.computed_fillet_continuation_status = None;
        }
        gesture.map_or_else(Vec::new, |gesture| {
            vec![EditorEffect::RestoreComputedFeatureContact {
                expected: gesture.expected,
                owner: gesture.owner,
                parent: gesture.parent,
                source: gesture.source,
                parameter: gesture.origin_parameter,
            }]
        })
    }

    /// Supplies the result of a host-projected temporary drag request. Rejection
    /// retains the prior finite preview; non-finite previews are ignored.
    pub fn projected_drag_result(
        &mut self,
        pointer_id: u64,
        request_id: u64,
        point: DesignPointId,
        accepted_model_position: Option<[f64; 2]>,
    ) -> Vec<EditorEffect> {
        let Some(gesture) = self.point_gesture else {
            return Vec::new();
        };
        if gesture.pointer_id != pointer_id
            || gesture.point != point
            || gesture.latest_request != Some(request_id)
            || !gesture.moved
        {
            return Vec::new();
        }
        let Some(position) =
            accepted_model_position.filter(|p| p.iter().all(|value| value.is_finite()))
        else {
            return Vec::new();
        };
        self.last_valid_drag_preview =
            Some((request_id, gesture.epoch, pointer_id, point, position));
        vec![EditorEffect::PreviewPointMove {
            point,
            model_position: position,
        }]
    }

    /// Supplies the result of one exact curve-control preview request.
    ///
    /// A rejected, stale, foreign, out-of-order, or non-finite result leaves the
    /// previous valid candidate untouched. Pointer release can therefore name
    /// only a request that the host independently accepted and retained.
    pub fn curve_control_preview_result(
        &mut self,
        pointer_id: u64,
        request_id: u64,
        expected: SketchDesignIdentity,
        control: DocumentCurveControlId,
        accepted_model_position: Option<[f64; 2]>,
    ) -> Vec<EditorEffect> {
        let Some(mut gesture) = self.curve_control_gesture else {
            return Vec::new();
        };
        if gesture.pointer_id != pointer_id
            || gesture.expected != expected
            || gesture.control != control
            || gesture.latest_request != Some(request_id)
            || !gesture.moved
        {
            return Vec::new();
        }
        let Some(position) = accepted_model_position
            .filter(|position| position.iter().all(|value| value.is_finite()))
        else {
            return Vec::new();
        };
        gesture.last_valid_request = Some((request_id, position));
        self.curve_control_gesture = Some(gesture);
        vec![EditorEffect::PreviewCurveControl {
            control,
            model_position: position,
        }]
    }

    pub(crate) fn projected_drag_request_disposition(
        &self,
        pointer_id: u64,
        request_id: u64,
        point: DesignPointId,
    ) -> ProjectedDragRequestDisposition {
        if let Some(gesture) = self.point_gesture
            && gesture.pointer_id == pointer_id
            && gesture.point == point
            && gesture.latest_request == Some(request_id)
            && gesture.moved
        {
            return ProjectedDragRequestDisposition::Current {
                gesture_epoch: gesture.epoch,
            };
        }
        if request_id < self.next_projection_request {
            ProjectedDragRequestDisposition::Stale
        } else {
            // Preserve the coordinator's direct synchronous API for native hosts
            // that do not route pointer input through this state machine.
            ProjectedDragRequestDisposition::Untracked
        }
    }

    pub(crate) fn curve_control_preview_request_disposition(
        &self,
        pointer_id: u64,
        request_id: u64,
        expected: SketchDesignIdentity,
        control: DocumentCurveControlId,
    ) -> CurveControlPreviewRequestDisposition {
        if let Some(gesture) = self.curve_control_gesture
            && gesture.pointer_id == pointer_id
            && gesture.expected == expected
            && gesture.control == control
            && gesture.latest_request == Some(request_id)
            && gesture.moved
        {
            return CurveControlPreviewRequestDisposition::Current;
        }
        if request_id < self.next_projection_request {
            CurveControlPreviewRequestDisposition::Stale
        } else {
            CurveControlPreviewRequestDisposition::Untracked
        }
    }

    pub(crate) fn point_gesture_snapshot(&self) -> Option<PointGestureSnapshot> {
        self.point_gesture.map(|gesture| PointGestureSnapshot {
            epoch: gesture.epoch,
            pointer_id: gesture.pointer_id,
            point: gesture.point,
        })
    }

    /// Completes a variable-length polyline or NURBS draft.
    pub fn complete_draft(&mut self, expected: SketchDesignIdentity) -> Vec<EditorEffect> {
        if self.pending_construction_commit.is_some() {
            return Vec::new();
        }
        let Some(draft) = self.draft.clone() else {
            return Vec::new();
        };
        let proposal = match draft.tool {
            EditorTool::Polyline | EditorTool::Nurbs => draft_proposal(&draft),
            _ => None,
        };
        let Some(proposal) = proposal else {
            self.draft_issue = Some(GeometryDraftIssue::CannotFinish);
            return Vec::new();
        };
        if !Self::draft_requires_construction_plan(&draft) {
            self.draft = None;
            self.draft_inference_engine.clear_session();
            let mut effects = self.clear_draft_inference_publication();
            effects.extend(commit_construction(expected, proposal, draft.geometry_role));
            return effects;
        }
        self.begin_construction_plan(expected, &draft, proposal)
    }

    /// Whether the current retained draft can be completed by an explicit Finish action.
    #[must_use]
    pub fn can_complete_draft(&self) -> bool {
        self.draft.as_ref().is_some_and(|draft| match draft.tool {
            EditorTool::Polyline | EditorTool::Nurbs => draft_proposal(draft).is_some(),
            _ => false,
        })
    }

    /// Returns semantic progress for the exact active geometry recipe.
    #[must_use]
    pub fn geometry_draft_status(&self) -> Option<GeometryDraftStatus> {
        let variant = self.geometry_tool_variant?;
        let completed_stages = self
            .draft
            .as_ref()
            .filter(|draft| draft.variant == variant)
            .map_or(0, |draft| draft.positions.len());
        let draft = self.draft.as_ref().filter(|draft| draft.variant == variant);
        Some(GeometryDraftStatus {
            variant,
            stage: geometry_draft_stage(variant, completed_stages)?,
            completed_stages,
            required_stages: geometry_variant_required_stages(variant),
            can_finish: self.can_complete_draft(),
            regularized: draft.is_some_and(|draft| draft.regularized),
            branch: GeometryDraftBranch {
                sweep: matches!(
                    variant,
                    GeometryToolVariant::CenterArc
                        | GeometryToolVariant::CenterAxesEllipticalArc
                        | GeometryToolVariant::AxisEndpointsEllipticalArc
                )
                .then_some(draft.map_or(self.conic_options.arc_sweep, |draft| {
                    draft.conic_options.arc_sweep
                })),
                hyperbola: (variant == GeometryToolVariant::Hyperbola).then_some(
                    draft.map_or(self.conic_options.hyperbola_branch, |draft| {
                        draft.conic_options.hyperbola_branch
                    }),
                ),
            },
            measurements: draft.map_or_else(Vec::new, geometry_draft_measurements),
            issue: self.draft_issue,
        })
    }

    /// Removes the most recent unfinished recipe operand without touching
    /// accepted document history.
    pub fn step_back_draft(&mut self) -> Vec<EditorEffect> {
        if self.pending_construction_commit.is_some() {
            return Vec::new();
        }
        let had_issue = self.draft_issue.take().is_some();
        let Some(mut draft) = self.draft.take() else {
            return had_issue
                .then_some(EditorEffect::ClearConstructionPreview)
                .into_iter()
                .collect();
        };
        if draft.positions.pop().is_none() {
            return Vec::new();
        }
        let _ = draft.points.pop();
        draft.closed = false;
        if draft.positions.is_empty() {
            draft.tangent_source = None;
        }
        let retained_stages = draft.positions.len();
        draft
            .confirmed_inference
            .retain(|confirmed| confirmed.stage_index < retained_stages);
        self.draft_inference_engine.clear_stage();
        let mut effects = self.clear_draft_inference_publication();
        if draft.positions.is_empty() {
            effects.push(EditorEffect::ClearConstructionPreview);
        } else {
            effects.extend(draft_preview(&draft).map(EditorEffect::PreviewConstruction));
            self.draft = Some(draft);
            self.prepare_next_draft_stage();
        }
        effects
    }

    /// Flips the complementary sweep owned by Center Arc or either
    /// elliptical-arc recipe without relying on pointer history.
    pub fn flip_geometry_draft_branch(&mut self) -> Vec<EditorEffect> {
        if self.pending_construction_commit.is_some() {
            return Vec::new();
        }
        let Some(draft) = self.draft.as_mut() else {
            return Vec::new();
        };
        if !matches!(
            draft.variant,
            GeometryToolVariant::CenterArc
                | GeometryToolVariant::CenterAxesEllipticalArc
                | GeometryToolVariant::AxisEndpointsEllipticalArc
        ) {
            return Vec::new();
        }
        draft.conic_options.arc_sweep = match draft.conic_options.arc_sweep {
            DocumentArcSweep::CounterClockwise => DocumentArcSweep::Clockwise,
            DocumentArcSweep::Clockwise => DocumentArcSweep::CounterClockwise,
        };
        self.draft_issue = None;
        self.conic_options.arc_sweep = draft.conic_options.arc_sweep;
        draft_preview(draft)
            .map(EditorEffect::PreviewConstruction)
            .into_iter()
            .collect()
    }

    /// First Escape cancels the current shape while retaining the exact tool;
    /// a draft-free second Escape activates Select.
    pub fn escape_geometry_tool(&mut self) -> Vec<EditorEffect> {
        if self.draft.is_some()
            || self.draft_issue.is_some()
            || self.point_gesture.is_some()
            || self.curve_control_gesture.is_some()
            || self.annotation_gesture.is_some()
            || self.feature_radius_gesture.is_some()
            || self.feature_contact_gesture.is_some()
        {
            self.cancel()
        } else if self.tool != EditorTool::Select {
            self.activate_tool(EditorTool::Select)
        } else {
            self.cancel()
        }
    }

    fn cancel_draft(&mut self) -> Vec<EditorEffect> {
        if self.pending_construction_commit.is_some() {
            return Vec::new();
        }
        let had_draft = self.draft.take().is_some();
        let had_issue = self.draft_issue.take().is_some();
        let mut effects = (had_draft || had_issue)
            .then_some(EditorEffect::ClearConstructionPreview)
            .into_iter()
            .collect::<Vec<_>>();
        self.draft_inference_engine.clear_session();
        effects.extend(self.clear_draft_inference_publication());
        effects
    }

    fn cancel_point_gesture(&mut self) -> Vec<EditorEffect> {
        let moved = self
            .point_gesture
            .take()
            .is_some_and(|gesture| gesture.moved);
        let had_preview = self.last_valid_drag_preview.take().is_some();
        (moved || had_preview)
            .then_some(EditorEffect::ClearPointPreview)
            .into_iter()
            .collect()
    }

    #[allow(
        clippy::too_many_lines,
        reason = "stage validation, preview retention, and tokenized terminal recovery are one auditable transition"
    )]
    fn draft_down(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
        authoring: DraftAuthoringInput,
    ) -> Vec<EditorEffect> {
        if !input.position.is_finite() || self.pending_construction_commit.is_some() {
            return Vec::new();
        }
        if self
            .draft
            .as_ref()
            .is_some_and(|draft| draft.pointer_id != input.pointer_id)
        {
            return Vec::new();
        }
        if self
            .draft
            .as_ref()
            .is_some_and(|draft| draft.prepared_input != scene.authenticated_prepared_input())
        {
            return self.clear_draft_inference_publication();
        }
        let prior_draft = self.draft.clone();
        let recovery_inference_engine = self.draft_inference_engine.clone();
        let stage_index = prior_draft.as_ref().map_or(0, |draft| draft.points.len());
        let active_variant = prior_draft
            .as_ref()
            .map(|draft| draft.variant)
            .or(self.geometry_tool_variant)
            .or_else(|| GeometryToolVariant::default_for_editor_tool(self.tool));
        let tangent_source = (active_variant == Some(GeometryToolVariant::TangentArc)
            && stage_index == 0)
            .then(|| self.resolve_tangent_arc_source(scene, input.position))
            .flatten();
        if active_variant == Some(GeometryToolVariant::TangentArc)
            && stage_index == 0
            && tangent_source.is_none()
        {
            return Vec::new();
        }
        let stage_result = if let Some(source) = tangent_source {
            Ok(Some(ResolvedDraftStage {
                operand: ConstructionPoint::New(source.position),
                position: source.position,
                confirmed: None,
                resolution: None,
            }))
        } else {
            self.resolve_draft_stage(
                scene,
                input.position,
                authoring.inference,
                self.tool,
                stage_index,
                prior_draft.as_ref(),
            )
        };
        let stage = match stage_result {
            Ok(Some(stage)) => stage,
            Ok(None) => return Vec::new(),
            Err(_) => {
                self.draft_inference_engine = recovery_inference_engine;
                return self.clear_draft_inference_publication();
            }
        };
        let ResolvedDraftStage {
            operand,
            position,
            confirmed,
            resolution,
        } = stage;
        if resolution
            .as_ref()
            .is_some_and(draft_inference_blocks_confirmation)
        {
            return self.publish_draft_inference(resolution);
        }
        let mut draft = prior_draft.clone().unwrap_or(Draft {
            tool: self.tool,
            variant: self
                .geometry_tool_variant
                .or_else(|| GeometryToolVariant::default_for_editor_tool(self.tool))
                .expect("every non-Select editor tool has an exact geometry variant"),
            exact_variant: self.exact_geometry_tool,
            geometry_role: self.authoring_geometry_role,
            prepared_input: scene.authenticated_prepared_input(),
            pointer_id: input.pointer_id,
            points: Vec::new(),
            positions: Vec::new(),
            confirmed_inference: Vec::new(),
            regularized: authoring.regularized,
            closed: false,
            tangent_source: None,
            conic_options: self.conic_options,
            nurbs_options: self.nurbs_options.clone(),
        });
        if draft.tool != self.tool {
            self.draft_inference_engine = recovery_inference_engine;
            return Vec::new();
        }
        self.draft_issue = None;
        draft.regularized = authoring.regularized;
        if tangent_source.is_some() {
            draft.tangent_source = tangent_source;
        }
        let closes_polyline = draft.variant == GeometryToolVariant::Polyline
            && draft.positions.len() >= 3
            && scene
                .viewport
                .model_to_screen(draft.positions[0])
                .distance(input.position)
                <= self.pick_tolerance.point_pixels;
        if closes_polyline {
            draft.closed = true;
        } else {
            draft.points.push(operand);
            draft.positions.push(position);
            if let Some(confirmed) = confirmed {
                draft.confirmed_inference.push(confirmed);
            }
        }
        if !valid_draft_stage(&draft) {
            self.draft_inference_engine = recovery_inference_engine;
            self.draft_issue = Some(GeometryDraftIssue::InvalidTerminalGeometry);
            return Vec::new();
        }
        let proposal = draft_proposal(&draft);
        let keep = matches!(
            draft.tool,
            EditorTool::Line
                | EditorTool::Rectangle
                | EditorTool::Circle
                | EditorTool::CounterClockwiseArc
                | EditorTool::QuadraticBezier
                | EditorTool::CubicBezier
                | EditorTool::Ellipse
                | EditorTool::EllipticalArc
                | EditorTool::RationalQuadraticConic
                | EditorTool::Parabola
                | EditorTool::Hyperbola
        ) && proposal.is_none()
            || (matches!(draft.tool, EditorTool::Polyline | EditorTool::Nurbs) && !draft.closed);
        if keep {
            let preview = draft_preview(&draft);
            self.draft = Some(draft);
            self.prepare_next_draft_stage();
            let mut effects = self.clear_draft_inference_publication();
            effects.extend(
                preview
                    .map(EditorEffect::PreviewConstruction)
                    .into_iter()
                    .collect::<Vec<_>>(),
            );
            effects
        } else {
            let Some(proposal) = proposal else {
                self.draft_inference_engine = recovery_inference_engine;
                return Vec::new();
            };
            if matches!(
                &proposal,
                ConstructionProposal::Point {
                    point: ConstructionPoint::Existing { .. }
                }
            ) {
                self.draft = None;
                self.draft_inference_engine.clear_session();
                return self.clear_draft_inference_publication();
            }
            if Self::draft_requires_construction_plan(&draft) {
                self.draft = prior_draft;
                self.begin_construction_plan_with_recovery(
                    scene.design_identity,
                    &draft,
                    proposal,
                    recovery_inference_engine,
                    resolution,
                )
            } else {
                self.draft = None;
                self.draft_inference_engine.clear_session();
                let mut effects = self.clear_draft_inference_publication();
                effects.extend(commit_construction(
                    scene.design_identity,
                    proposal,
                    draft.geometry_role,
                ));
                effects
            }
        }
    }

    fn draft_requires_construction_plan(draft: &Draft) -> bool {
        draft.exact_variant
            || !draft.confirmed_inference.is_empty()
            || recipe_relations(draft).is_some_and(|relations| !relations.is_empty())
    }

    fn draft_move(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
        authoring: DraftAuthoringInput,
    ) -> Vec<EditorEffect> {
        if self.pending_construction_commit.is_some() {
            return Vec::new();
        }
        let Some(draft) = self.draft.as_ref() else {
            let Some(variant) = self
                .geometry_tool_variant
                .or_else(|| GeometryToolVariant::default_for_editor_tool(self.tool))
            else {
                return self.clear_draft_inference_publication();
            };
            if variant == GeometryToolVariant::TangentArc {
                let preview =
                    self.resolve_tangent_arc_source(scene, input.position)
                        .map(|source| {
                            EditorEffect::PreviewConstruction(ConstructionPreview::Anchor {
                                position: source.position,
                            })
                        });
                let mut effects = self.clear_draft_inference_publication();
                if let Some(preview) = preview {
                    effects.push(preview);
                } else {
                    effects.push(EditorEffect::ClearConstructionPreview);
                }
                return effects;
            }
            if construction_stage_semantics_for(self.exact_geometry_tool, self.tool, variant, 0)
                .and_then(|semantics| semantics.coordinate_role.inference_subject())
                .is_none()
            {
                return self.clear_draft_inference_publication();
            }
            let resolution = self.resolve_draft_inference(
                scene,
                input.position,
                authoring.inference,
                self.tool,
                0,
                None,
            );
            return if let Ok(resolution) = resolution {
                self.publish_draft_inference(resolution)
            } else {
                self.draft_inference_engine.clear_session();
                self.clear_draft_inference_publication()
            };
        };
        if draft.pointer_id != input.pointer_id
            || draft.prepared_input != scene.authenticated_prepared_input()
            || !input.position.is_finite()
        {
            return Vec::new();
        }
        let draft = draft.clone();
        let stage_index = draft.points.len();
        let recovery_inference_engine = self.draft_inference_engine.clone();
        let stage = match self.resolve_draft_stage(
            scene,
            input.position,
            authoring.inference,
            draft.tool,
            stage_index,
            Some(&draft),
        ) {
            Ok(Some(stage)) => stage,
            Ok(None) => return Vec::new(),
            Err(_) => {
                self.draft_inference_engine = recovery_inference_engine;
                return self.clear_draft_inference_publication();
            }
        };
        let mut preview = draft;
        preview.regularized = authoring.regularized;
        preview.points.push(stage.operand);
        preview.positions.push(stage.position);
        if let Some(confirmed) = stage.confirmed {
            preview.confirmed_inference.push(confirmed);
        }
        let correction_is_valid = draft_proposal(&preview)
            .and_then(|proposal| construction_commit_plan(&preview, proposal).ok())
            .is_some();
        if let Some(retained) = self.draft.as_mut() {
            retained.regularized = authoring.regularized;
            if correction_is_valid {
                self.draft_issue = None;
            }
        }
        let mut effects = self.publish_draft_inference(stage.resolution);
        effects.extend(
            draft_preview(&preview)
                .map(EditorEffect::PreviewConstruction)
                .into_iter()
                .collect::<Vec<_>>(),
        );
        effects
    }

    fn resolve_draft_stage(
        &mut self,
        scene: &EditorScene,
        pointer: ScreenPoint,
        input: DraftInferenceInput,
        tool: EditorTool,
        stage_index: usize,
        draft: Option<&Draft>,
    ) -> Result<Option<ResolvedDraftStage>, DraftInferenceError> {
        let raw_position = scene.viewport.screen_to_model(pointer);
        if !raw_position.into_iter().all(f64::is_finite) {
            return Ok(None);
        }
        let variant = draft
            .map(|draft| draft.variant)
            .or(self.geometry_tool_variant)
            .or_else(|| GeometryToolVariant::default_for_editor_tool(tool))
            .ok_or(DraftInferenceError::InvalidFrame)?;
        let exact_variant = draft.map_or(self.exact_geometry_tool, |draft| draft.exact_variant);
        let Some(subject) =
            construction_stage_semantics_for(exact_variant, tool, variant, stage_index)
                .and_then(|semantics| semantics.coordinate_role.inference_subject())
        else {
            return Ok(Some(ResolvedDraftStage {
                operand: ConstructionPoint::New(raw_position),
                position: raw_position,
                confirmed: None,
                resolution: None,
            }));
        };

        let resolution =
            self.resolve_draft_inference(scene, pointer, input, tool, stage_index, draft)?;
        let Some(candidate) = resolution
            .as_ref()
            .and_then(resolved_draft_inference_candidate)
            .cloned()
        else {
            return Ok(Some(ResolvedDraftStage {
                operand: ConstructionPoint::New(raw_position),
                position: raw_position,
                confirmed: None,
                resolution,
            }));
        };
        let point_identity = candidate
            .relations
            .iter()
            .find_map(|relation| match relation {
                DraftInferenceRelation::PointIdentity { point } => Some(*point),
                DraftInferenceRelation::CoincidentWithOrigin
                | DraftInferenceRelation::PointOnDatumAxis { .. }
                | DraftInferenceRelation::PointOnCurve { .. }
                | DraftInferenceRelation::PointOnCreatedCurve { .. }
                | DraftInferenceRelation::Midpoint { .. }
                | DraftInferenceRelation::Horizontal
                | DraftInferenceRelation::Vertical
                | DraftInferenceRelation::Parallel { .. }
                | DraftInferenceRelation::Perpendicular { .. }
                | DraftInferenceRelation::HorizontalPoints { .. }
                | DraftInferenceRelation::VerticalPoints { .. }
                | DraftInferenceRelation::HorizontalPointToMidpoint { .. }
                | DraftInferenceRelation::VerticalPointToMidpoint { .. }
                | DraftInferenceRelation::Concentric { .. }
                | DraftInferenceRelation::Collinear { .. } => None,
            });
        let position = candidate.adjusted_model_position;
        let operand = if subject.is_point_operand()
            && let Some(id) = point_identity
        {
            let accepted_position = candidate.references.iter().find_map(|reference| {
                if let DraftReferenceAnchor::PersistentPoint {
                    point,
                    model_position,
                    ..
                } = reference
                    && *point == id
                {
                    return Some(*model_position);
                }
                None
            });
            let Some(accepted_position) = accepted_position else {
                return Err(DraftInferenceError::InvalidFrame);
            };
            ConstructionPoint::Existing {
                id,
                position: accepted_position,
            }
        } else {
            ConstructionPoint::New(position)
        };
        let resolution_ref = resolution
            .as_ref()
            .ok_or(DraftInferenceError::InvalidFrame)?;
        let confirmed = confirmed_draft_inference(resolution_ref, candidate, stage_index)?;
        Ok(Some(ResolvedDraftStage {
            operand,
            position,
            confirmed: Some(confirmed),
            resolution,
        }))
    }

    fn resolve_tangent_arc_source(
        &self,
        scene: &EditorScene,
        pointer: ScreenPoint,
    ) -> Option<TangentArcSource> {
        tangent_arc_source_at(
            scene,
            pointer,
            self.pick_tolerance.point_pixels,
            self.geometry_policy,
        )
    }

    fn resolve_draft_inference(
        &mut self,
        scene: &EditorScene,
        pointer: ScreenPoint,
        input: DraftInferenceInput,
        tool: EditorTool,
        stage_index: usize,
        draft: Option<&Draft>,
    ) -> Result<Option<DraftInferenceResolution>, DraftInferenceError> {
        let variant = draft
            .map(|draft| draft.variant)
            .or(self.geometry_tool_variant)
            .or_else(|| GeometryToolVariant::default_for_editor_tool(tool))
            .ok_or(DraftInferenceError::InvalidFrame)?;
        let exact_variant = draft.map_or(self.exact_geometry_tool, |draft| draft.exact_variant);
        let Some(semantics) =
            construction_stage_semantics_for(exact_variant, tool, variant, stage_index)
        else {
            return Ok(None);
        };
        let Some(subject) = semantics.coordinate_role.inference_subject() else {
            return Ok(None);
        };
        if !pointer.is_finite() {
            return Ok(None);
        }
        let span_start = semantics
            .directional_span
            .then(|| draft.and_then(|draft| draft.positions.last().copied()))
            .flatten();
        let scene_inputs = if input.suppressed {
            DraftInferenceSceneInputs {
                anchors: Vec::new(),
                semantic_centers: Vec::new(),
            }
        } else {
            match scene.draft_inference_scene_inputs(
                pointer,
                subject,
                self.draft_inference_engine.policy().limits,
            ) {
                DraftInferenceSceneInputCollection::Complete(inputs) => inputs,
                DraftInferenceSceneInputCollection::ResourceLimited(evidence) => {
                    self.draft_inference_engine.clear_stage();
                    let raw_model = scene.viewport.screen_to_model(pointer);
                    return Ok(Some(DraftInferenceResolution {
                        status: DraftInferenceStatus::ResourceLimited,
                        completeness: DraftInferenceCompleteness::SceneLimit(evidence),
                        raw_model_position: raw_model,
                        adjusted_model_position: raw_model,
                        raw_screen_position: pointer,
                        adjusted_screen_position: pointer,
                        candidates: Vec::new(),
                        guides: Vec::new(),
                    }));
                }
            }
        };
        let frame = DraftInferenceFrame::from_scene_with_semantic_centers(
            scene,
            self.geometry_policy,
            DraftInferenceSample {
                raw_screen_position: pointer,
                subject,
                span_start,
            },
            scene_inputs.anchors,
            scene_inputs.semantic_centers,
        );
        self.draft_inference_engine.resolve(&frame, input).map(Some)
    }

    fn publish_draft_inference(
        &mut self,
        resolution: Option<DraftInferenceResolution>,
    ) -> Vec<EditorEffect> {
        let resolution = resolution.filter(draft_inference_is_publishable);
        if self.draft_inference_resolution == resolution {
            return Vec::new();
        }
        self.draft_inference_resolution.clone_from(&resolution);
        vec![EditorEffect::DraftInferenceChanged(resolution)]
    }

    fn prepare_next_draft_stage(&mut self) {
        let references = self
            .draft
            .as_ref()
            .filter(|draft| {
                draft
                    .points
                    .len()
                    .checked_sub(1)
                    .and_then(|stage_index| {
                        construction_stage_semantics_for(
                            draft.exact_variant,
                            draft.tool,
                            draft.variant,
                            stage_index,
                        )
                    })
                    .is_some_and(|semantics| semantics.reference_handoff)
            })
            .and_then(|draft| draft.confirmed_inference.last())
            .map(confirmed_positional_references)
            .unwrap_or_default();
        self.draft_inference_engine.clear_stage();
        for reference in references {
            let _ = self.draft_inference_engine.remember_reference(reference);
        }
    }

    fn begin_construction_plan(
        &mut self,
        expected: SketchDesignIdentity,
        draft: &Draft,
        proposal: ConstructionProposal,
    ) -> Vec<EditorEffect> {
        self.begin_construction_plan_with_recovery(
            expected,
            draft,
            proposal,
            self.draft_inference_engine.clone(),
            self.draft_inference_resolution.clone(),
        )
    }

    fn begin_construction_plan_with_recovery(
        &mut self,
        expected: SketchDesignIdentity,
        draft: &Draft,
        proposal: ConstructionProposal,
        recovery_inference_engine: DraftInferenceEngine,
        resolution: Option<DraftInferenceResolution>,
    ) -> Vec<EditorEffect> {
        let plan = match construction_commit_plan(draft, proposal) {
            Ok(plan) => plan,
            Err(error) => {
                self.draft_inference_engine = recovery_inference_engine;
                self.draft_issue = Some(match error {
                    ConstructionPlanError::IncompatibleConstraintIntent => {
                        GeometryDraftIssue::IncompatibleConstraintIntent
                    }
                    ConstructionPlanError::InvalidDraftState => {
                        GeometryDraftIssue::ConstructionRejected
                    }
                });
                return Vec::new();
            }
        };
        let Some(prepared_input) = draft
            .prepared_input
            .filter(|input| input.design_identity() == expected)
        else {
            self.draft_inference_engine = recovery_inference_engine;
            return Vec::new();
        };
        let token = ConstructionCommitToken(self.next_construction_commit_token);
        let Some(next_token) = self.next_construction_commit_token.checked_add(1) else {
            self.draft_inference_engine = recovery_inference_engine;
            return Vec::new();
        };
        self.next_construction_commit_token = next_token;
        self.pending_construction_commit = Some(PendingConstructionCommit {
            token,
            expected: Box::new(prepared_input),
            plan: plan.clone(),
            recovery_inference_engine,
        });
        self.draft_issue = None;
        let mut effects = self.publish_draft_inference(resolution);
        effects.extend(draft_preview(draft).map(EditorEffect::PreviewConstruction));
        effects.push(EditorEffect::CommitConstructionPlan {
            expected: Box::new(prepared_input),
            token,
            plan,
        });
        effects
    }

    /// Returns the token awaiting host publication acknowledgement, if any.
    #[must_use]
    pub fn pending_construction_commit_token(&self) -> Option<ConstructionCommitToken> {
        self.pending_construction_commit
            .as_ref()
            .map(|pending| pending.token)
    }

    pub(crate) fn authenticates_construction_commit(
        &self,
        token: ConstructionCommitToken,
        expected: &PreparedSketchInput,
        plan: &ConstructionCommitPlan,
    ) -> bool {
        self.pending_construction_commit
            .as_ref()
            .is_some_and(|pending| {
                pending.token == token
                    && pending.expected.as_ref() == expected
                    && pending.plan == *plan
            })
    }

    pub(crate) fn pending_construction_plan_matches(
        &self,
        expected: &PreparedSketchInput,
        plan: &ConstructionCommitPlan,
    ) -> bool {
        self.pending_construction_commit
            .as_ref()
            .is_some_and(|pending| pending.expected.as_ref() == expected && pending.plan == *plan)
    }

    /// Completes or rejects one tokenized atomic construction publication.
    ///
    /// Success consumes the retained draft and clears its preview. Rejection
    /// restores the exact pre-terminal inference state while leaving the
    /// terminal preview visible, so the next pointer move replaces only the
    /// rejected terminal candidate.
    pub fn acknowledge_construction_commit(
        &mut self,
        token: ConstructionCommitToken,
        accepted: bool,
    ) -> Vec<EditorEffect> {
        if self
            .pending_construction_commit
            .as_ref()
            .is_none_or(|pending| pending.token != token)
        {
            return Vec::new();
        }
        let Some(pending) = self.pending_construction_commit.take() else {
            return Vec::new();
        };
        if !accepted {
            self.draft_inference_engine = pending.recovery_inference_engine;
            self.draft_issue = Some(GeometryDraftIssue::ConstructionRejected);
            return Vec::new();
        }
        self.draft = None;
        self.draft_issue = None;
        self.draft_inference_engine.clear_session();
        let mut effects = vec![EditorEffect::ClearConstructionPreview];
        effects.extend(self.clear_draft_inference_publication());
        effects
    }

    pub(crate) fn invalidate_for_retained_state_change(&mut self, force: bool) {
        let _ = self.invalidate_pointer_context();
        self.curve_control_gesture = None;
        let preserve_pending_ack = !force && self.pending_construction_commit.is_some();
        if force {
            self.pending_construction_commit = None;
        }
        self.draft_issue = None;
        if !preserve_pending_ack {
            self.draft = None;
        }
        self.draft_inference_engine.clear_session();
        if !preserve_pending_ack {
            self.draft_inference_resolution = None;
        }
    }
}

fn commit_construction(
    expected: SketchDesignIdentity,
    proposal: ConstructionProposal,
    role: GeometryRole,
) -> Vec<EditorEffect> {
    vec![
        EditorEffect::CommitConstruction {
            expected,
            proposal,
            role,
        },
        EditorEffect::ClearConstructionPreview,
    ]
}

fn resolved_draft_inference_candidate(
    resolution: &DraftInferenceResolution,
) -> Option<&DraftInferenceCandidate> {
    let DraftInferenceStatus::Resolved { candidate } = resolution.status else {
        return None;
    };
    resolution
        .candidates
        .iter()
        .find(|value| value.id == candidate)
}

fn confirmed_draft_inference(
    resolution: &DraftInferenceResolution,
    candidate: DraftInferenceCandidate,
    stage_index: usize,
) -> Result<ConfirmedDraftInference, DraftInferenceError> {
    let selected_candidate = resolved_draft_inference_candidate(resolution)
        .filter(|selected| *selected == &candidate)
        .ok_or(DraftInferenceError::InvalidFrame)?;
    let candidate_guides_are_owned = candidate.guides.iter().enumerate().all(|(index, guide)| {
        guide.id.candidate == Some(candidate.id)
            && usize::try_from(guide.id.ordinal).is_ok_and(|ordinal| ordinal == index)
    });
    let published_candidate_guides = resolution
        .guides
        .iter()
        .filter(|guide| guide.id.candidate.is_some())
        .copied()
        .collect::<Vec<_>>();
    if candidate.id.get() == 0
        || !candidate_guides_are_owned
        || published_candidate_guides != selected_candidate.guides
    {
        return Err(DraftInferenceError::InvalidFrame);
    }
    Ok(ConfirmedDraftInference {
        candidate_id: candidate.id,
        stage_index,
        relations: candidate.relations,
        references: candidate.references,
    })
}

fn draft_inference_is_publishable(resolution: &DraftInferenceResolution) -> bool {
    !resolution.guides.is_empty()
        || matches!(
            resolution.status,
            DraftInferenceStatus::Resolved { .. }
                | DraftInferenceStatus::Ambiguous { .. }
                | DraftInferenceStatus::Suppressed
                | DraftInferenceStatus::ResourceLimited
                | DraftInferenceStatus::StalePreferredCandidate { .. }
        )
}

fn draft_inference_blocks_confirmation(resolution: &DraftInferenceResolution) -> bool {
    matches!(
        resolution.status,
        DraftInferenceStatus::Ambiguous { .. }
            | DraftInferenceStatus::ResourceLimited
            | DraftInferenceStatus::StalePreferredCandidate { .. }
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConstructionCoordinateRole {
    PointOperand,
    CenteredPointOperand { prospective_curve_index: usize },
    CircleCircumference,
    CoordinateOnly,
}

impl ConstructionCoordinateRole {
    const fn inference_subject(self) -> Option<DraftInferenceSubject> {
        match self {
            Self::PointOperand => Some(DraftInferenceSubject::PointOperand),
            Self::CenteredPointOperand {
                prospective_curve_index,
            } => Some(DraftInferenceSubject::CenteredPointOperand {
                prospective_curve_index,
            }),
            Self::CircleCircumference => Some(DraftInferenceSubject::CircleCircumference),
            Self::CoordinateOnly => None,
        }
    }
}

/// Unified semantics for one coordinate in a tool's construction sequence.
///
/// A present descriptor denotes a valid stage. Coordinate-only stages remain
/// distinct from an absent (invalid) stage even though neither publishes draft
/// inference. Point ordinals describe proposal allocation order independently
/// of coordinate order, which matters for interleaved conic coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ConstructionStageSemantics {
    coordinate_role: ConstructionCoordinateRole,
    point_operand_ordinal: Option<usize>,
    directional_span: bool,
    completed_span: Option<DraftSpanSlot>,
    reference_handoff: bool,
}

impl ConstructionStageSemantics {
    const fn point_operand(point_operand_ordinal: usize) -> Self {
        Self {
            coordinate_role: ConstructionCoordinateRole::PointOperand,
            point_operand_ordinal: Some(point_operand_ordinal),
            directional_span: false,
            completed_span: None,
            reference_handoff: false,
        }
    }

    const fn centered_point_operand(
        point_operand_ordinal: usize,
        prospective_curve_index: usize,
    ) -> Self {
        Self {
            coordinate_role: ConstructionCoordinateRole::CenteredPointOperand {
                prospective_curve_index,
            },
            point_operand_ordinal: Some(point_operand_ordinal),
            directional_span: false,
            completed_span: None,
            reference_handoff: false,
        }
    }

    const fn coordinate_only(coordinate_role: ConstructionCoordinateRole) -> Self {
        Self {
            coordinate_role,
            point_operand_ordinal: None,
            directional_span: false,
            completed_span: None,
            reference_handoff: false,
        }
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "the exact M78 recipe table is intentionally exhaustive and auditable"
)]
fn geometry_variant_stage_semantics(
    variant: GeometryToolVariant,
    stage_index: usize,
) -> Option<ConstructionStageSemantics> {
    use GeometryToolVariant as V;

    let point = ConstructionStageSemantics::point_operand;
    let prospective = || {
        ConstructionStageSemantics::coordinate_only(ConstructionCoordinateRole::CircleCircumference)
    };
    match variant {
        V::SketchPoint => (stage_index == 0).then(|| point(0)),
        V::Segment | V::MidpointLine => match stage_index {
            0 => Some(ConstructionStageSemantics {
                reference_handoff: true,
                ..point(0)
            }),
            1 => Some(ConstructionStageSemantics {
                directional_span: true,
                completed_span: Some(DraftSpanSlot::Created {
                    curve_index: 0,
                    segment: 0,
                }),
                ..point(1)
            }),
            _ => None,
        },
        V::Polyline => {
            let mut semantics = ConstructionStageSemantics {
                reference_handoff: true,
                ..point(stage_index)
            };
            if stage_index > 0 {
                semantics.directional_span = true;
                semantics.completed_span =
                    u32::try_from(stage_index - 1)
                        .ok()
                        .map(|segment| DraftSpanSlot::Created {
                            curve_index: 0,
                            segment,
                        });
            }
            Some(semantics)
        }
        V::TwoPointAlignedRectangle => (stage_index < 2).then(|| point(stage_index)),
        V::ThreePointCornerRectangle => (stage_index < 3).then(|| point(stage_index)),
        V::CenterRectangle => match stage_index {
            0 => Some(ConstructionStageSemantics::centered_point_operand(0, 0)),
            1 => Some(point(1)),
            _ => None,
        },
        V::ThreePointCenterRectangle => match stage_index {
            0 => Some(ConstructionStageSemantics::centered_point_operand(0, 0)),
            1 => Some(ConstructionStageSemantics::coordinate_only(
                ConstructionCoordinateRole::CoordinateOnly,
            )),
            2 => Some(point(1)),
            _ => None,
        },
        V::CenterRadiusCircle => match stage_index {
            0 => Some(ConstructionStageSemantics::centered_point_operand(0, 0)),
            1 => Some(prospective()),
            _ => None,
        },
        V::TwoPointDiameterCircle => (stage_index < 2).then(prospective),
        V::ThreePointCircle | V::ThreePointArc => (stage_index < 3).then(prospective),
        V::CenterArc => match stage_index {
            0 => Some(ConstructionStageSemantics::centered_point_operand(0, 0)),
            1 | 2 => Some(prospective()),
            _ => None,
        },
        V::TangentArc => match stage_index {
            0 => Some(ConstructionStageSemantics::coordinate_only(
                ConstructionCoordinateRole::CoordinateOnly,
            )),
            1 => Some(prospective()),
            _ => None,
        },
        V::CenterAxesEllipse => match stage_index {
            0 => Some(ConstructionStageSemantics::centered_point_operand(0, 0)),
            1 => Some(point(1)),
            2 => Some(ConstructionStageSemantics::coordinate_only(
                ConstructionCoordinateRole::CoordinateOnly,
            )),
            _ => None,
        },
        V::AxisEndpointsEllipse => match stage_index {
            0 => Some(point(0)),
            1 => Some(prospective()),
            2 => Some(ConstructionStageSemantics::coordinate_only(
                ConstructionCoordinateRole::CoordinateOnly,
            )),
            _ => None,
        },
        V::CenterAxesEllipticalArc => match stage_index {
            0 => Some(ConstructionStageSemantics::centered_point_operand(0, 0)),
            1 => Some(point(1)),
            2 => Some(ConstructionStageSemantics::coordinate_only(
                ConstructionCoordinateRole::CoordinateOnly,
            )),
            3 | 4 => Some(prospective()),
            _ => None,
        },
        V::AxisEndpointsEllipticalArc => match stage_index {
            0 => Some(point(0)),
            // The opposite major-axis pole need not belong to the bounded arc.
            // Keep this construction sample coordinate-only until the recipe can
            // model associativity against the untrimmed support ellipse.
            1 | 2 => Some(ConstructionStageSemantics::coordinate_only(
                ConstructionCoordinateRole::CoordinateOnly,
            )),
            3 | 4 => Some(prospective()),
            _ => None,
        },
        V::QuadraticBezier => (stage_index < 3).then(|| point(stage_index)),
        V::CubicBezier => (stage_index < 4).then(|| point(stage_index)),
        V::RationalQuadraticConic => match stage_index {
            0 => Some(point(0)),
            1 => Some(ConstructionStageSemantics::coordinate_only(
                ConstructionCoordinateRole::CoordinateOnly,
            )),
            2 => Some(point(1)),
            _ => None,
        },
        V::Parabola | V::Hyperbola => (stage_index < 2).then(|| point(stage_index)),
        V::OpenControlNurbs | V::PeriodicControlNurbs => Some(point(stage_index)),
    }
}

fn construction_stage_semantics_for(
    exact_variant: bool,
    tool: EditorTool,
    variant: GeometryToolVariant,
    stage_index: usize,
) -> Option<ConstructionStageSemantics> {
    if exact_variant {
        geometry_variant_stage_semantics(variant, stage_index)
    } else {
        construction_stage_semantics(tool, stage_index)
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive table keeps construction-coordinate semantics auditable"
)]
fn construction_stage_semantics(
    tool: EditorTool,
    stage_index: usize,
) -> Option<ConstructionStageSemantics> {
    match tool {
        EditorTool::Select => None,
        EditorTool::Point => {
            (stage_index == 0).then(|| ConstructionStageSemantics::point_operand(0))
        }
        EditorTool::Line => match stage_index {
            0 => Some(ConstructionStageSemantics {
                reference_handoff: true,
                ..ConstructionStageSemantics::point_operand(0)
            }),
            1 => Some(ConstructionStageSemantics {
                directional_span: true,
                completed_span: Some(DraftSpanSlot::Created {
                    curve_index: 0,
                    segment: 0,
                }),
                ..ConstructionStageSemantics::point_operand(1)
            }),
            _ => None,
        },
        EditorTool::Polyline => {
            let mut semantics = ConstructionStageSemantics {
                reference_handoff: true,
                ..ConstructionStageSemantics::point_operand(stage_index)
            };
            if stage_index > 0 {
                semantics.directional_span = true;
                semantics.completed_span =
                    u32::try_from(stage_index - 1)
                        .ok()
                        .map(|segment| DraftSpanSlot::Created {
                            curve_index: 0,
                            segment,
                        });
            }
            Some(semantics)
        }
        EditorTool::Rectangle => (stage_index < 2).then(|| {
            ConstructionStageSemantics::coordinate_only(ConstructionCoordinateRole::CoordinateOnly)
        }),
        EditorTool::Circle => match stage_index {
            0 => Some(ConstructionStageSemantics::centered_point_operand(0, 0)),
            1 => Some(ConstructionStageSemantics::coordinate_only(
                ConstructionCoordinateRole::CircleCircumference,
            )),
            _ => None,
        },
        EditorTool::CounterClockwiseArc => match stage_index {
            0 => Some(ConstructionStageSemantics::centered_point_operand(0, 0)),
            1 | 2 => Some(ConstructionStageSemantics::coordinate_only(
                ConstructionCoordinateRole::CoordinateOnly,
            )),
            _ => None,
        },
        EditorTool::QuadraticBezier => {
            (stage_index < 3).then(|| ConstructionStageSemantics::point_operand(stage_index))
        }
        EditorTool::CubicBezier => {
            (stage_index < 4).then(|| ConstructionStageSemantics::point_operand(stage_index))
        }
        EditorTool::Ellipse | EditorTool::Hyperbola => match stage_index {
            0 => Some(ConstructionStageSemantics::centered_point_operand(0, 0)),
            1 => Some(ConstructionStageSemantics::point_operand(1)),
            _ => None,
        },
        EditorTool::EllipticalArc => match stage_index {
            0 => Some(ConstructionStageSemantics::centered_point_operand(0, 0)),
            1 => Some(ConstructionStageSemantics::point_operand(1)),
            2 | 3 => Some(ConstructionStageSemantics::coordinate_only(
                ConstructionCoordinateRole::CoordinateOnly,
            )),
            _ => None,
        },
        EditorTool::RationalQuadraticConic => match stage_index {
            0 => Some(ConstructionStageSemantics::point_operand(0)),
            1 => Some(ConstructionStageSemantics::coordinate_only(
                ConstructionCoordinateRole::CoordinateOnly,
            )),
            2 => Some(ConstructionStageSemantics::point_operand(1)),
            _ => None,
        },
        EditorTool::Parabola => {
            (stage_index < 2).then(|| ConstructionStageSemantics::point_operand(stage_index))
        }
        EditorTool::Nurbs => Some(ConstructionStageSemantics::point_operand(stage_index)),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConstructionPlanError {
    InvalidDraftState,
    IncompatibleConstraintIntent,
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive lowering keeps every authenticated inference relation in a single atomic construction plan"
)]
fn construction_commit_plan(
    draft: &Draft,
    proposal: ConstructionProposal,
) -> Result<ConstructionCommitPlan, ConstructionPlanError> {
    let mut relations = recipe_relations(draft).ok_or(ConstructionPlanError::InvalidDraftState)?;
    for confirmed in &draft.confirmed_inference {
        if confirmed.candidate_id.get() == 0 {
            return Err(ConstructionPlanError::InvalidDraftState);
        }
        if !recipe_preserves_stage_point_identity(draft, &proposal, confirmed.stage_index) {
            continue;
        }
        let stage_semantics = construction_stage_semantics_for(
            draft.exact_variant,
            draft.tool,
            draft.variant,
            confirmed.stage_index,
        );
        for relation in confirmed.relations.iter().copied() {
            match relation {
                DraftInferenceRelation::PointIdentity { point: expected } => {
                    let point = draft_point_slot(draft, confirmed.stage_index)
                        .ok_or(ConstructionPlanError::InvalidDraftState)?;
                    if point != DraftPointSlot::Existing(expected) {
                        return Err(ConstructionPlanError::InvalidDraftState);
                    }
                }
                DraftInferenceRelation::CoincidentWithOrigin => {
                    relations.push(InferredRelation::CoincidentWithOrigin {
                        point: draft_point_slot(draft, confirmed.stage_index)
                            .ok_or(ConstructionPlanError::InvalidDraftState)?,
                    });
                }
                DraftInferenceRelation::PointOnDatumAxis { axis } => {
                    relations.push(InferredRelation::PointOnDatumAxis {
                        point: draft_point_slot(draft, confirmed.stage_index)
                            .ok_or(ConstructionPlanError::InvalidDraftState)?,
                        axis,
                    });
                }
                DraftInferenceRelation::PointOnCurve { contact } => {
                    let point = draft_point_slot(draft, confirmed.stage_index)
                        .ok_or(ConstructionPlanError::InvalidDraftState)?;
                    relations.push(InferredRelation::PointOnCurve {
                        point,
                        contact: DraftContactDescriptor {
                            span: DraftSpanSlot::Existing(contact.span),
                            domain: contact.domain,
                            parameter: contact.parameter,
                            winding: contact.winding,
                            neighborhood: contact.neighborhood,
                        },
                    });
                }
                DraftInferenceRelation::PointOnCreatedCurve { point } => {
                    if let Some(contact) = created_curve_contact(&proposal, confirmed, point) {
                        relations.push(InferredRelation::PointOnCurve {
                            point: DraftPointSlot::Existing(point),
                            contact,
                        });
                    } else {
                        return Err(ConstructionPlanError::IncompatibleConstraintIntent);
                    }
                }
                DraftInferenceRelation::Midpoint { span } => {
                    let point = draft_point_slot(draft, confirmed.stage_index)
                        .ok_or(ConstructionPlanError::InvalidDraftState)?;
                    relations.push(InferredRelation::Midpoint {
                        point,
                        line: DraftSpanSlot::Existing(span),
                    });
                }
                DraftInferenceRelation::Horizontal => {
                    relations.push(InferredRelation::Horizontal {
                        line: stage_semantics
                            .and_then(|semantics| semantics.completed_span)
                            .ok_or(ConstructionPlanError::InvalidDraftState)?,
                    });
                }
                DraftInferenceRelation::Vertical => {
                    relations.push(InferredRelation::Vertical {
                        line: stage_semantics
                            .and_then(|semantics| semantics.completed_span)
                            .ok_or(ConstructionPlanError::InvalidDraftState)?,
                    });
                }
                DraftInferenceRelation::Parallel { reference } => {
                    relations.push(InferredRelation::Parallel {
                        first: stage_semantics
                            .and_then(|semantics| semantics.completed_span)
                            .ok_or(ConstructionPlanError::InvalidDraftState)?,
                        second: DraftSpanSlot::Existing(reference),
                    });
                }
                DraftInferenceRelation::Perpendicular { reference } => {
                    relations.push(InferredRelation::Perpendicular {
                        first: stage_semantics
                            .and_then(|semantics| semantics.completed_span)
                            .ok_or(ConstructionPlanError::InvalidDraftState)?,
                        second: DraftSpanSlot::Existing(reference),
                    });
                }
                DraftInferenceRelation::HorizontalPoints { reference } => {
                    relations.push(InferredRelation::HorizontalPoints {
                        first: draft_point_slot(draft, confirmed.stage_index)
                            .ok_or(ConstructionPlanError::InvalidDraftState)?,
                        second: DraftPointSlot::Existing(reference),
                    });
                }
                DraftInferenceRelation::VerticalPoints { reference } => {
                    relations.push(InferredRelation::VerticalPoints {
                        first: draft_point_slot(draft, confirmed.stage_index)
                            .ok_or(ConstructionPlanError::InvalidDraftState)?,
                        second: DraftPointSlot::Existing(reference),
                    });
                }
                DraftInferenceRelation::HorizontalPointToMidpoint { reference } => {
                    relations.push(InferredRelation::HorizontalPointToMidpoint {
                        point: draft_point_slot(draft, confirmed.stage_index)
                            .ok_or(ConstructionPlanError::InvalidDraftState)?,
                        line: DraftSpanSlot::Existing(reference),
                    });
                }
                DraftInferenceRelation::VerticalPointToMidpoint { reference } => {
                    relations.push(InferredRelation::VerticalPointToMidpoint {
                        point: draft_point_slot(draft, confirmed.stage_index)
                            .ok_or(ConstructionPlanError::InvalidDraftState)?,
                        line: DraftSpanSlot::Existing(reference),
                    });
                }
                DraftInferenceRelation::Concentric {
                    reference,
                    prospective_curve_index,
                } => {
                    relations.push(InferredRelation::Concentric {
                        first: DraftCurveSlot::Created {
                            curve_index: prospective_curve_index,
                        },
                        second: DraftCurveSlot::Existing(reference),
                    });
                }
                DraftInferenceRelation::Collinear { reference } => {
                    relations.push(InferredRelation::Collinear {
                        first: DraftLineSupportSlot {
                            span: stage_semantics
                                .and_then(|semantics| semantics.completed_span)
                                .ok_or(ConstructionPlanError::InvalidDraftState)?,
                            direction: DocumentDirectionSense::Forward,
                        },
                        second: DraftLineSupportSlot {
                            span: DraftSpanSlot::Existing(reference),
                            direction: DocumentDirectionSense::Forward,
                        },
                    });
                }
            }
        }
    }
    Ok(ConstructionCommitPlan {
        proposal,
        role: draft.geometry_role,
        relations,
    })
}

fn created_span(curve_index: usize) -> DraftSpanSlot {
    DraftSpanSlot::Created {
        curve_index,
        segment: 0,
    }
}

fn recipe_relations(draft: &Draft) -> Option<Vec<InferredRelation>> {
    use GeometryToolVariant as V;

    let mut relations = Vec::new();
    if !draft.exact_variant {
        return Some(relations);
    }
    match draft.variant {
        V::MidpointLine => relations.push(InferredRelation::Midpoint {
            point: draft_point_slot(draft, 0)?,
            line: created_span(0),
        }),
        V::TangentArc => {
            let source = draft.tangent_source?;
            relations.push(InferredRelation::CurveCurveTangency {
                first: DraftContactDescriptor {
                    span: DraftSpanSlot::Existing(source.contact.support.span),
                    domain: source.domain,
                    parameter: source.contact.parameter,
                    winding: source.contact.support.winding,
                    neighborhood: source.contact.neighborhood,
                },
                second: DraftContactDescriptor {
                    span: created_span(0),
                    domain: ContactDomain::Bounded {
                        lower: 0.0,
                        upper: 1.0,
                    },
                    parameter: 0.0,
                    winding: 0,
                    neighborhood: ContactNeighborhood::Start,
                },
                orientation: source.orientation,
            });
        }
        V::TwoPointAlignedRectangle | V::CenterRectangle => {
            relations.extend([
                InferredRelation::Horizontal {
                    line: created_span(0),
                },
                InferredRelation::Vertical {
                    line: created_span(1),
                },
                InferredRelation::Horizontal {
                    line: created_span(2),
                },
                InferredRelation::Vertical {
                    line: created_span(3),
                },
            ]);
        }
        V::ThreePointCornerRectangle | V::ThreePointCenterRectangle => {
            relations.extend([
                InferredRelation::Perpendicular {
                    first: created_span(0),
                    second: created_span(1),
                },
                InferredRelation::Parallel {
                    first: created_span(0),
                    second: created_span(2),
                },
                InferredRelation::Parallel {
                    first: created_span(1),
                    second: created_span(3),
                },
            ]);
        }
        _ => {}
    }
    if matches!(
        draft.variant,
        V::CenterRectangle | V::ThreePointCenterRectangle
    ) {
        relations.push(InferredRelation::Midpoint {
            point: draft_point_slot(draft, 0)?,
            line: created_span(4),
        });
    }
    if draft.regularized
        && matches!(
            draft.variant,
            V::TwoPointAlignedRectangle
                | V::ThreePointCornerRectangle
                | V::CenterRectangle
                | V::ThreePointCenterRectangle
        )
    {
        relations.push(InferredRelation::EqualLength {
            first: created_span(0),
            second: created_span(1),
        });
    }
    Some(relations)
}

fn recipe_preserves_stage_point_identity(
    draft: &Draft,
    proposal: &ConstructionProposal,
    stage_index: usize,
) -> bool {
    let Some(ConstructionPoint::Existing { id: expected, .. }) = draft.points.get(stage_index)
    else {
        return true;
    };
    let proposal_point = match proposal {
        ConstructionProposal::RectangleLoop { points, .. } => match draft.variant {
            GeometryToolVariant::TwoPointAlignedRectangle
            | GeometryToolVariant::ThreePointCornerRectangle
            | GeometryToolVariant::CenterRectangle => points.get(stage_index),
            GeometryToolVariant::ThreePointCenterRectangle => match stage_index {
                0 => points.first(),
                2 => points.get(1),
                _ => None,
            },
            _ => None,
        },
        ConstructionProposal::MidpointLine {
            center, endpoint, ..
        } => match stage_index {
            0 => Some(center),
            1 => Some(endpoint),
            _ => None,
        },
        _ => return true,
    };
    matches!(proposal_point, Some(ConstructionPoint::Existing { id, .. }) if id == expected)
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive created-family contact authenticator keeps incidence admission auditable"
)]
fn created_curve_contact(
    proposal: &ConstructionProposal,
    confirmed: &ConfirmedDraftInference,
    point: DesignPointId,
) -> Option<DraftContactDescriptor> {
    let target = confirmed
        .references
        .iter()
        .find_map(|reference| match reference {
            DraftReferenceAnchor::PersistentPoint {
                point: candidate,
                model_position,
                ..
            } if *candidate == point => Some(*model_position),
            _ => None,
        })?;
    if !target.into_iter().all(f64::is_finite) {
        return None;
    }
    let (domain, parameter, neighborhood) = match proposal {
        ConstructionProposal::Circle { center, radius } => {
            let center = center.position();
            let delta = [target[0] - center[0], target[1] - center[1]];
            let target_radius = delta[0].hypot(delta[1]);
            let parameter = delta[1].atan2(delta[0]).rem_euclid(std::f64::consts::TAU);
            if !center.into_iter().all(f64::is_finite)
                || !radius.is_finite()
                || *radius <= 0.0
                || !target_radius.is_finite()
                || target_radius <= 0.0
                || !parameter.is_finite()
                || !positive_lengths_match(*radius, target_radius)
            {
                return None;
            }
            (
                ContactDomain::Periodic {
                    period: std::f64::consts::TAU,
                },
                parameter,
                ContactNeighborhood::Interior,
            )
        }
        ConstructionProposal::CounterClockwiseArc { center, start, end } => {
            bounded_circular_arc_contact(
                center.position(),
                *start,
                *end,
                DocumentArcSweep::CounterClockwise,
                target,
            )?
        }
        ConstructionProposal::CircularArc {
            center,
            start,
            end,
            sweep,
        } => bounded_circular_arc_contact(center.position(), *start, *end, *sweep, target)?,
        ConstructionProposal::Ellipse {
            center,
            major_axis_point,
            minor_axis_ratio,
        }
        | ConstructionProposal::AxisEndpointEllipse {
            center,
            major_axis_point,
            minor_axis_ratio,
        } => {
            let parameter = ellipse_support_parameter(
                center.position(),
                major_axis_point.position(),
                *minor_axis_ratio,
                target,
            )?;
            (
                ContactDomain::Periodic {
                    period: std::f64::consts::TAU,
                },
                parameter,
                ContactNeighborhood::Interior,
            )
        }
        ConstructionProposal::EllipticalArc {
            center,
            major_axis_point,
            minor_axis_ratio,
            start_angle,
            end_angle,
            sweep,
        }
        | ConstructionProposal::AxisEndpointEllipticalArc {
            center,
            major_axis_point,
            minor_axis_ratio,
            start_angle,
            end_angle,
            sweep,
        } => {
            let target_angle = ellipse_support_parameter(
                center.position(),
                major_axis_point.position(),
                *minor_axis_ratio,
                target,
            )?;
            bounded_sweep_contact(*start_angle, *end_angle, *sweep, target_angle)?
        }
        _ => return None,
    };
    Some(DraftContactDescriptor {
        span: DraftSpanSlot::Created {
            curve_index: 0,
            segment: 0,
        },
        domain,
        parameter,
        winding: 0,
        neighborhood,
    })
}

fn bounded_circular_arc_contact(
    center: [f64; 2],
    start: [f64; 2],
    end: [f64; 2],
    sweep: DocumentArcSweep,
    target: [f64; 2],
) -> Option<(ContactDomain, f64, ContactNeighborhood)> {
    if !center.into_iter().all(f64::is_finite)
        || !start.into_iter().all(f64::is_finite)
        || !end.into_iter().all(f64::is_finite)
        || !target.into_iter().all(f64::is_finite)
    {
        return None;
    }
    let start_radius = (start[0] - center[0]).hypot(start[1] - center[1]);
    let end_radius = (end[0] - center[0]).hypot(end[1] - center[1]);
    let target_radius = (target[0] - center[0]).hypot(target[1] - center[1]);
    if !positive_lengths_match(start_radius, end_radius)
        || !positive_lengths_match(start_radius, target_radius)
    {
        return None;
    }
    let start_angle = (start[1] - center[1]).atan2(start[0] - center[0]);
    let end_angle = (end[1] - center[1]).atan2(end[0] - center[0]);
    let target_angle = (target[1] - center[1]).atan2(target[0] - center[0]);
    bounded_sweep_contact(start_angle, end_angle, sweep, target_angle)
}

fn positive_lengths_match(first: f64, second: f64) -> bool {
    const RELATIVE_TOLERANCE: f64 = 1.0e-9;
    first.is_finite()
        && second.is_finite()
        && first > 0.0
        && second > 0.0
        && (first - second).abs() <= RELATIVE_TOLERANCE * first.max(second)
}

fn bounded_sweep_contact(
    start: f64,
    end: f64,
    sweep: DocumentArcSweep,
    target: f64,
) -> Option<(ContactDomain, f64, ContactNeighborhood)> {
    if ![start, end, target].into_iter().all(f64::is_finite) {
        return None;
    }
    let (total, offset) = match sweep {
        DocumentArcSweep::CounterClockwise => (
            (end - start).rem_euclid(std::f64::consts::TAU),
            (target - start).rem_euclid(std::f64::consts::TAU),
        ),
        DocumentArcSweep::Clockwise => (
            (start - end).rem_euclid(std::f64::consts::TAU),
            (start - target).rem_euclid(std::f64::consts::TAU),
        ),
    };
    if !(total.is_finite() && total > 0.0 && offset.is_finite()) {
        return None;
    }
    let tolerance = 1.0e-10 * total.max(1.0);
    if offset > total + tolerance {
        return None;
    }
    let parameter = (offset / total).clamp(0.0, 1.0);
    let neighborhood = if parameter <= 1.0e-10 {
        ContactNeighborhood::Start
    } else if parameter >= 1.0 - 1.0e-10 {
        ContactNeighborhood::End
    } else {
        ContactNeighborhood::Interior
    };
    Some((
        ContactDomain::Bounded {
            lower: 0.0,
            upper: 1.0,
        },
        parameter,
        neighborhood,
    ))
}

fn ellipse_support_parameter(
    center: [f64; 2],
    major_axis_point: [f64; 2],
    minor_axis_ratio: f64,
    target: [f64; 2],
) -> Option<f64> {
    if !center.into_iter().all(f64::is_finite)
        || !major_axis_point.into_iter().all(f64::is_finite)
        || !target.into_iter().all(f64::is_finite)
        || !minor_axis_ratio.is_finite()
        || minor_axis_ratio <= 0.0
    {
        return None;
    }
    let axis = [
        major_axis_point[0] - center[0],
        major_axis_point[1] - center[1],
    ];
    let major = axis[0].hypot(axis[1]);
    if !(major.is_finite() && major > 0.0) {
        return None;
    }
    let unit = [axis[0] / major, axis[1] / major];
    let normal = [-unit[1], unit[0]];
    let delta = [target[0] - center[0], target[1] - center[1]];
    let x = delta[0].mul_add(unit[0], delta[1] * unit[1]) / major;
    let y = delta[0].mul_add(normal[0], delta[1] * normal[1]) / (major * minor_axis_ratio);
    let support_residual = x.mul_add(x, y * y) - 1.0;
    if !x.is_finite()
        || !y.is_finite()
        || !support_residual.is_finite()
        || support_residual.abs() > 1.0e-9
    {
        return None;
    }
    Some(y.atan2(x).rem_euclid(std::f64::consts::TAU))
}

fn confirmed_positional_references(
    confirmed: &ConfirmedDraftInference,
) -> Vec<DraftReferenceAnchor> {
    let mut references = Vec::new();
    for relation in &confirmed.relations {
        let reference =
            confirmed
                .references
                .iter()
                .copied()
                .find(|reference| match (relation, reference) {
                    (
                        DraftInferenceRelation::PointIdentity { point: expected }
                        | DraftInferenceRelation::PointOnCreatedCurve { point: expected }
                        | DraftInferenceRelation::HorizontalPoints {
                            reference: expected,
                        }
                        | DraftInferenceRelation::VerticalPoints {
                            reference: expected,
                        },
                        DraftReferenceAnchor::PersistentPoint { point, .. },
                    ) => *point == *expected,
                    (
                        DraftInferenceRelation::HorizontalPointToMidpoint {
                            reference: expected,
                        }
                        | DraftInferenceRelation::VerticalPointToMidpoint {
                            reference: expected,
                        }
                        | DraftInferenceRelation::Midpoint { span: expected },
                        DraftReferenceAnchor::Midpoint { span, .. },
                    ) => *span == *expected,
                    (
                        DraftInferenceRelation::PointOnCurve { contact: expected },
                        DraftReferenceAnchor::CurvePoint { contact, .. }
                        | DraftReferenceAnchor::AffineSupport { contact, .. },
                    ) => *contact == *expected,
                    _ => false,
                });
        if let Some(reference) = reference
            && !references.contains(&reference)
        {
            references.push(reference);
        }
    }
    references
}

fn tangent_arc_source_at(
    scene: &EditorScene,
    pointer: ScreenPoint,
    tolerance_pixels: f64,
    policy: GeometryInteractionPolicy,
) -> Option<TangentArcSource> {
    const SHARED_ENDPOINT_TOLERANCE_PIXELS: f64 = 1.0e-6;

    if !pointer.is_finite() || !tolerance_pixels.is_finite() || tolerance_pixels < 0.0 {
        return None;
    }
    let mut candidates = Vec::<(f64, TangentArcSource)>::new();
    for curve in scene.accepted_document.curves() {
        for endpoint in [FeatureEndpoint::Start, FeatureEndpoint::End] {
            let endpoint = DocumentEndpointRef {
                curve: curve.id,
                endpoint,
            };
            let Some(source) = tangent_arc_source_candidate(scene, endpoint, policy) else {
                continue;
            };
            let distance = scene
                .viewport
                .model_to_screen(source.position)
                .distance(pointer);
            if distance.is_finite() && distance <= tolerance_pixels {
                candidates.push((distance, source));
            }
        }
    }
    candidates.sort_by(|(first_distance, first), (second_distance, second)| {
        first_distance
            .total_cmp(second_distance)
            .then_with(|| first.contact.support.span.cmp(&second.contact.support.span))
            .then_with(|| {
                tangent_endpoint_order(first.endpoint.endpoint)
                    .cmp(&tangent_endpoint_order(second.endpoint.endpoint))
            })
    });
    let (_, best) = candidates.first().copied()?;
    let best_screen = scene.viewport.model_to_screen(best.position);
    // A geometrically shared endpoint has no unique support under a point-only
    // gesture. Do not let persistent ID ordering silently choose the tangency
    // parent; a future explicit support picker can broaden this contract.
    if candidates.iter().skip(1).any(|(_, candidate)| {
        best_screen.distance(scene.viewport.model_to_screen(candidate.position))
            <= SHARED_ENDPOINT_TOLERANCE_PIXELS
    }) {
        return None;
    }
    Some(best)
}

fn tangent_arc_source_candidate(
    scene: &EditorScene,
    endpoint: DocumentEndpointRef,
    policy: GeometryInteractionPolicy,
) -> Option<TangentArcSource> {
    let contact = scene
        .accepted_document
        .curve_endpoint_contact_seed(endpoint)
        .ok()?;
    let (domain, painted_endpoint) = scene.curves.iter().find_map(|curve| {
        if curve.span != contact.support.span
            || !curve.authoring_eligible
            || !curve.is_interactive(policy)
            || !matches!(curve.origin, SceneCurveOrigin::Native)
            || curve.screen_parameters.len() != curve.screen_polyline.len()
        {
            return None;
        }
        let ContactDomain::Bounded { lower, upper } = curve.contact_domain else {
            return None;
        };
        if !lower.is_finite()
            || !upper.is_finite()
            || lower > contact.parameter
            || contact.parameter > upper
        {
            return None;
        }
        let painted = match endpoint.endpoint {
            FeatureEndpoint::Start => curve
                .screen_parameters
                .first()
                .zip(curve.screen_polyline.first()),
            FeatureEndpoint::End => curve
                .screen_parameters
                .last()
                .zip(curve.screen_polyline.last()),
        }?;
        (painted.0.to_bits() == contact.parameter.to_bits() && painted.1.is_finite())
            .then_some((curve.contact_domain, *painted.1))
    })?;
    let jet = scene
        .accepted_document
        .evaluate_curve_jet(contact.support.span, contact.parameter)
        .ok()?;
    let differential = jet.differential().ok()?;
    let position = [jet.position.x, jet.position.y];
    let parameter_tangent = [differential.unit_tangent.x, differential.unit_tangent.y];
    if !position.into_iter().all(f64::is_finite)
        || !parameter_tangent.into_iter().all(f64::is_finite)
        || scene
            .viewport
            .model_to_screen(position)
            .distance(painted_endpoint)
            > 1.0e-6
    {
        return None;
    }
    let (outgoing_tangent, orientation, expected_neighborhood) = match endpoint.endpoint {
        FeatureEndpoint::Start => (
            [-parameter_tangent[0], -parameter_tangent[1]],
            TangentOrientation::Opposed,
            ContactNeighborhood::Start,
        ),
        FeatureEndpoint::End => (
            parameter_tangent,
            TangentOrientation::Aligned,
            ContactNeighborhood::End,
        ),
    };
    if contact.neighborhood != expected_neighborhood {
        return None;
    }
    Some(TangentArcSource {
        endpoint,
        contact,
        domain,
        position,
        outgoing_tangent,
        orientation,
    })
}

const fn tangent_endpoint_order(endpoint: FeatureEndpoint) -> u8 {
    match endpoint {
        FeatureEndpoint::Start => 0,
        FeatureEndpoint::End => 1,
    }
}

fn draft_point_slot(draft: &Draft, stage_index: usize) -> Option<DraftPointSlot> {
    construction_stage_semantics_for(draft.exact_variant, draft.tool, draft.variant, stage_index)?
        .point_operand_ordinal?;
    match *draft.points.get(stage_index)? {
        ConstructionPoint::Existing { id, .. } => Some(DraftPointSlot::Existing(id)),
        ConstructionPoint::New(_) => {
            let point_index = (0..stage_index)
                .filter(|index| {
                    construction_stage_semantics_for(
                        draft.exact_variant,
                        draft.tool,
                        draft.variant,
                        *index,
                    )
                    .and_then(|semantics| semantics.point_operand_ordinal)
                    .is_some()
                })
                .filter(|index| matches!(draft.points.get(*index), Some(ConstructionPoint::New(_))))
                .count();
            Some(DraftPointSlot::Created { point_index })
        }
    }
}

/// Compact selection-sensitive authoring vocabulary.
///
/// An intent is not an equation identity. The headless coordinator resolves it
/// to one [`ResolvedConstraintKind`] from typed selected operands.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstraintIntent {
    Lock,
    Coincident,
    Horizontal,
    Vertical,
    Parallel,
    Perpendicular,
    Equal,
    Midpoint,
    Symmetric,
    Tangent,
    Continuity,
    Concentric,
    Collinear,
}

/// Exact persistent constraint family selected by contextual dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedConstraintKind {
    FixedPoint,
    CoincidentWithOrigin,
    PointOnDatumAxis,
    CoincidentPoints,
    PointOnCurve,
    CurveContact,
    HorizontalLine,
    VerticalLine,
    HorizontalPoints,
    VerticalPoints,
    ConcentricCurves,
    CollinearSupports,
    CollinearWithDatumAxis,
    ParallelLines,
    PerpendicularLines,
    RadialLine,
    EqualLength,
    EqualRadius,
    EqualCurvature,
    Midpoint,
    SymmetricAboutLine,
    SymmetricAboutDatumAxis,
    CurveTangency,
    EndpointContinuity,
}

impl ResolvedConstraintKind {
    /// Selection-specific presentation label; equations remain domain-owned.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::FixedPoint => "Lock point",
            Self::CoincidentWithOrigin => "Coincident with Origin",
            Self::PointOnDatumAxis => "Point on datum axis",
            Self::CoincidentPoints => "Coincident",
            Self::PointOnCurve => "Point on curve",
            Self::CurveContact => "Curve contact",
            Self::HorizontalLine => "Horizontal",
            Self::VerticalLine => "Vertical",
            Self::HorizontalPoints => "Horizontal points",
            Self::VerticalPoints => "Vertical points",
            Self::ConcentricCurves => "Concentric",
            Self::CollinearSupports => "Collinear",
            Self::CollinearWithDatumAxis => "Collinear with datum axis",
            Self::ParallelLines => "Parallel",
            Self::PerpendicularLines => "Perpendicular",
            Self::RadialLine => "Normal to circle / arc",
            Self::EqualLength => "Equal length",
            Self::EqualRadius => "Equal radius",
            Self::EqualCurvature => "Equal curvature",
            Self::Midpoint => "Midpoint",
            Self::SymmetricAboutLine => "Symmetric about line",
            Self::SymmetricAboutDatumAxis => "Symmetric about datum axis",
            Self::CurveTangency => "Tangent",
            Self::EndpointContinuity => "Endpoint continuity",
        }
    }
}

/// Complete M55 alpha dimension action vocabulary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DimensionKind {
    PointDistance,
    SegmentLength,
    Radius,
    Diameter,
    OrientedAngle,
}

/// Explicit branch state for one newly constructed curve contact.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactActionChoice {
    pub support: DocumentCurveSpanRef,
    pub domain: ContactDomain,
    pub parameter: f64,
    pub neighborhood: ContactNeighborhood,
    pub tangent_orientation: Option<TangentOrientation>,
}

/// Typed request for one relation action over the coordinator's current selection.
#[derive(Clone, Debug, PartialEq)]
pub struct ConstraintActionRequest {
    pub intent: ConstraintIntent,
    pub label: String,
    pub contacts: Vec<ContactActionChoice>,
    pub relation: Option<ConstraintRelationChoice>,
}

/// Explicit non-contact branch state for a contextual relation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConstraintRelationChoice {
    EqualCurvature(DocumentCurveCurvatureRelation),
    Continuity(DocumentCurveContinuity),
}

/// Typed request for one dimension action over the coordinator's current selection.
#[derive(Clone, Debug, PartialEq)]
pub struct DimensionActionRequest {
    pub kind: DimensionKind,
    pub mode: DocumentDimensionMode,
    pub label: String,
    pub angle_orientation: DocumentAngleOrientation,
}

/// Headless branch-choice metadata for one action operand.
#[derive(Clone, Debug, PartialEq)]
pub enum ActionChoice {
    Contact {
        operand: u8,
        span: CurveSpan,
        domains: Vec<ContactDomain>,
        default_parameter: f64,
        neighborhoods: Vec<ContactNeighborhood>,
        tangent_orientations: Vec<TangentOrientation>,
        default_winding: i32,
    },
    AngleOrientation {
        values: Vec<DocumentAngleOrientation>,
    },
    EqualCurvature {
        values: Vec<DocumentCurveCurvatureRelation>,
    },
    Continuity {
        values: Vec<DocumentCurveContinuity>,
    },
}

/// Headless editor validation or public scene-evaluation failure.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EditorError {
    #[error("viewport must have finite positive dimensions and scale")]
    InvalidViewport,
    #[error("interaction tolerances must be finite and non-negative")]
    InvalidTolerance,
    #[error("invalid construction options: {0}")]
    InvalidConstructionOptions(&'static str),
    #[error("computed-feature snapshot does not match the supplied accepted sketch")]
    StaleComputedFeatureSnapshot,
    #[error("prepared sketch input does not match the supplied accepted scene")]
    StalePreparedSketchInput,
    #[error("computed-feature interaction affordance is missing, stale, or malformed")]
    InvalidComputedFeatureAffordance,
    #[error(transparent)]
    Document(#[from] geosolve_sketch::DocumentError),
    #[error(transparent)]
    Curve(#[from] geosolve_sketch::DocumentCurveEvaluationError),
    #[error(transparent)]
    CurveControl(#[from] geosolve_sketch::DocumentCurveControlError),
}

fn build_native_scene_curves(
    accepted_document: &SketchDocument,
    design_document: Option<&SketchDocument>,
    viewport: Viewport,
    chord_tolerance_pixels: f64,
) -> Result<Vec<SceneCurve>, EditorError> {
    let mut curves = Vec::new();
    for curve in accepted_document.curves() {
        let role = accepted_document
            .geometry_role(curve.id)
            .unwrap_or_default();
        for span in accepted_document.curve_spans(curve.id)? {
            for interval in accepted_document.visible_intervals(span)? {
                let start = accepted_document.evaluate_curve_jet(span, interval.start)?;
                let end = accepted_document.evaluate_curve_jet(span, interval.end)?;
                let start = viewport.model_to_screen([start.position.x, start.position.y]);
                let end = viewport.model_to_screen([end.position.x, end.position.y]);
                let mut screen_polyline = vec![start];
                let mut screen_parameters = vec![interval.start];
                tessellate(
                    accepted_document,
                    viewport,
                    span,
                    interval.start,
                    start,
                    interval.end,
                    end,
                    chord_tolerance_pixels,
                    0,
                    &mut screen_polyline,
                    &mut screen_parameters,
                )?;
                curves.push(SceneCurve {
                    span,
                    authoring_eligible: design_document.is_none_or(|design| {
                        design
                            .curve_spans(span.curve)
                            .is_ok_and(|spans| spans.contains(&span))
                    }),
                    affine: is_linear_span(accepted_document, span),
                    contact_domain: painted_contact_domain(accepted_document, span)?,
                    role,
                    source_role: role,
                    origin: SceneCurveOrigin::Native,
                    screen_polyline,
                    screen_parameters,
                    drag_handle_point: match &curve.definition {
                        CurveDefinition::Circle { center, .. }
                        | CurveDefinition::CircularArc { center, .. } => Some(*center),
                        _ => None,
                    },
                });
            }
        }
    }
    Ok(curves)
}

fn scene_curve_for_interval(
    document: &SketchDocument,
    viewport: Viewport,
    span: CurveSpan,
    start_parameter: f64,
    end_parameter: f64,
    tolerance: f64,
) -> Result<SceneCurve, EditorError> {
    if !start_parameter.is_finite()
        || !end_parameter.is_finite()
        || end_parameter <= start_parameter
    {
        return Err(EditorError::StaleComputedFeatureSnapshot);
    }
    let start = document.evaluate_curve_jet(span, start_parameter)?;
    let end = document.evaluate_curve_jet(span, end_parameter)?;
    let start = viewport.model_to_screen([start.position.x, start.position.y]);
    let end = viewport.model_to_screen([end.position.x, end.position.y]);
    let mut screen_polyline = vec![start];
    let mut screen_parameters = vec![start_parameter];
    tessellate(
        document,
        viewport,
        span,
        start_parameter,
        start,
        end_parameter,
        end,
        tolerance,
        0,
        &mut screen_polyline,
        &mut screen_parameters,
    )?;
    let drag_handle_point = document
        .curve(span.curve)
        .and_then(|curve| match &curve.definition {
            CurveDefinition::Circle { center, .. }
            | CurveDefinition::CircularArc { center, .. } => Some(*center),
            _ => None,
        });
    Ok(SceneCurve {
        span,
        authoring_eligible: true,
        affine: is_linear_span(document, span),
        contact_domain: painted_contact_domain(document, span)?,
        role: document.geometry_role(span.curve).unwrap_or_default(),
        source_role: document.geometry_role(span.curve).unwrap_or_default(),
        origin: SceneCurveOrigin::Native,
        screen_polyline,
        screen_parameters,
        drag_handle_point,
    })
}

fn painted_contact_domain(
    document: &SketchDocument,
    span: CurveSpan,
) -> Result<ContactDomain, EditorError> {
    document
        .curve_contact_domains(span)?
        .into_iter()
        .find(|domain| !matches!(domain, ContactDomain::SupportingLine))
        .ok_or(EditorError::InvalidConstructionOptions(
            "native curve span has no painted contact domain",
        ))
}

// The segment count is explicitly clamped to 4096 before allocation, and every
// loop index is therefore exactly representable as `f64`.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn tessellate_computed_arc(
    arc: &geosolve_sketch_features::ComputedCircularArc,
    viewport: Viewport,
    chord_tolerance_pixels: f64,
) -> Result<Vec<ScreenPoint>, EditorError> {
    Ok(tessellate_computed_arc_geometry(arc, viewport, chord_tolerance_pixels)?.screen_polyline)
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn tessellate_computed_arc_geometry(
    arc: &geosolve_sketch_features::ComputedCircularArc,
    viewport: Viewport,
    chord_tolerance_pixels: f64,
) -> Result<SceneFilletAlternativeGeometry, EditorError> {
    if !arc.center.into_iter().all(f64::is_finite)
        || !arc.radius.is_finite()
        || arc.radius <= 0.0
        || !arc.start_angle.is_finite()
        || !arc.end_angle.is_finite()
    {
        return Err(EditorError::StaleComputedFeatureSnapshot);
    }
    let tau = std::f64::consts::TAU;
    let delta = match arc.sweep {
        DocumentArcSweep::CounterClockwise => (arc.end_angle - arc.start_angle).rem_euclid(tau),
        DocumentArcSweep::Clockwise => -(arc.start_angle - arc.end_angle).rem_euclid(tau),
    };
    if !delta.is_finite() || delta.abs() <= f64::EPSILON {
        return Err(EditorError::StaleComputedFeatureSnapshot);
    }
    let screen_radius = arc.radius * viewport.pixels_per_model_unit;
    let cosine = (1.0 - chord_tolerance_pixels / screen_radius).clamp(-1.0, 1.0);
    let max_step = (2.0 * cosine.acos()).clamp(1.0e-3, std::f64::consts::FRAC_PI_4);
    let segments = ((delta.abs() / max_step).ceil() as usize)
        .clamp(usize::from(MIN_COMPUTED_ARC_SEGMENTS), 4096);
    let model_polyline = (0..=segments)
        .map(|index| {
            let fraction = index as f64 / segments as f64;
            let angle = delta.mul_add(fraction, arc.start_angle);
            [
                arc.radius.mul_add(angle.cos(), arc.center[0]),
                arc.radius.mul_add(angle.sin(), arc.center[1]),
            ]
        })
        .collect::<Vec<_>>();
    let screen_polyline = model_polyline
        .iter()
        .copied()
        .map(|point| viewport.model_to_screen(point))
        .collect();
    let geometry = SceneFilletAlternativeGeometry {
        model_polyline,
        screen_polyline,
    };
    if !geometry.is_valid(viewport) {
        return Err(EditorError::StaleComputedFeatureSnapshot);
    }
    Ok(geometry)
}

#[allow(clippy::too_many_arguments)]
fn tessellate(
    document: &SketchDocument,
    viewport: Viewport,
    span: CurveSpan,
    start_parameter: f64,
    start: ScreenPoint,
    end_parameter: f64,
    end: ScreenPoint,
    tolerance: f64,
    depth: u8,
    output: &mut Vec<ScreenPoint>,
    parameters: &mut Vec<f64>,
) -> Result<(), EditorError> {
    let middle_parameter = (start_parameter + end_parameter) * 0.5;
    let middle = document.evaluate_curve_jet(span, middle_parameter)?;
    let middle = viewport.model_to_screen([middle.position.x, middle.position.y]);
    let chord_middle = ScreenPoint {
        x: (start.x + end.x) * 0.5,
        y: (start.y + end.y) * 0.5,
    };
    let needs_curved_baseline =
        !is_linear_span(document, span) && depth < MIN_CURVED_TESSELLATION_DEPTH;
    if depth < MAX_TESSELLATION_DEPTH
        && (needs_curved_baseline || middle.distance(chord_middle) > tolerance)
    {
        tessellate(
            document,
            viewport,
            span,
            start_parameter,
            start,
            middle_parameter,
            middle,
            tolerance,
            depth + 1,
            output,
            parameters,
        )?;
        tessellate(
            document,
            viewport,
            span,
            middle_parameter,
            middle,
            end_parameter,
            end,
            tolerance,
            depth + 1,
            output,
            parameters,
        )?;
    } else {
        output.push(end);
        parameters.push(end_parameter);
    }
    Ok(())
}

fn point_hit(point: &ScenePoint, position: ScreenPoint, tolerance_pixels: f64) -> Option<Hit> {
    let distance = position.distance(point.screen_position);
    (distance <= tolerance_pixels).then_some(Hit {
        item: SelectionItem::Point(point.id),
        distance_pixels: distance,
        curve_parameter: None,
        geometry: Some(SceneGeometryHit::Point {
            incidence: point.role_incidence,
        }),
    })
}

#[derive(Clone, Copy, Debug)]
struct SceneCurvePointerSample {
    total_parameter: f64,
    model_position: [f64; 2],
    distance_pixels: f64,
}

#[derive(Clone, Copy, Debug)]
struct SceneCurveChordProjection {
    segment_index: usize,
    distance_pixels: f64,
    parameter: f64,
    lower_parameter: f64,
    upper_parameter: f64,
}

fn draft_inference_scene_resource_limit(
    scene: &EditorScene,
    limits: DraftInferenceLimits,
) -> Option<DraftInferenceSceneLimit> {
    let eligible_curves = scene.curves.iter().filter(|curve| curve.authoring_eligible);
    let prospective_anchors = eligible_curves
        .clone()
        .fold(scene.construction_snap_points.len(), |count, curve| {
            count.saturating_add(if curve.affine { 2 } else { 1 })
        });
    if prospective_anchors > limits.max_scene_anchors {
        return Some(DraftInferenceSceneLimit {
            resource: DraftInferenceSceneResource::Anchors,
            required: prospective_anchors,
            limit: limits.max_scene_anchors,
        });
    }
    let curve_segments = eligible_curves.fold(0usize, |count, curve| {
        count.saturating_add(curve.screen_polyline.len().saturating_sub(1))
    });
    (curve_segments > limits.max_scene_curve_segments).then_some(DraftInferenceSceneLimit {
        resource: DraftInferenceSceneResource::CurveSegments,
        required: curve_segments,
        limit: limits.max_scene_curve_segments,
    })
}

fn append_nonlinear_draft_anchors(
    document: &SketchDocument,
    samples: &mut Vec<(&SceneCurve, DraftReferenceOrigin, SceneCurvePointerSample)>,
    anchors: &mut Vec<DraftReferenceAnchor>,
    anchor_limit: usize,
) -> Result<(), DraftInferenceSceneLimit> {
    samples.sort_by(|first, second| {
        first
            .0
            .span
            .cmp(&second.0.span)
            .then_with(|| first.2.total_parameter.total_cmp(&second.2.total_parameter))
            .then_with(|| first.2.distance_pixels.total_cmp(&second.2.distance_pixels))
    });
    let mut span_start = 0;
    while span_start < samples.len() {
        let span = samples[span_start].0.span;
        let span_end = samples[span_start..]
            .iter()
            .position(|(curve, _, _)| curve.span != span)
            .map_or(samples.len(), |offset| span_start + offset);
        let Some(nearest) = samples[span_start..span_end]
            .iter()
            .map(|(_, _, sample)| sample.distance_pixels)
            .min_by(f64::total_cmp)
        else {
            span_start = span_end;
            continue;
        };
        let close_to_nearest = |sample: &SceneCurvePointerSample| {
            sample.distance_pixels <= nearest + CURVE_BRANCH_CANDIDATE_BAND_PIXELS
        };
        let candidate_count = samples[span_start..span_end]
            .iter()
            .filter(|(_, _, sample)| close_to_nearest(sample))
            .count();
        let required = anchors.len().saturating_add(candidate_count);
        if required > anchor_limit {
            return Err(DraftInferenceSceneLimit {
                resource: DraftInferenceSceneResource::Anchors,
                required,
                limit: anchor_limit,
            });
        }

        let mut branch_ordinal = 0u32;
        let mut previous_parameter: Option<f64> = None;
        for (curve, origin, sample) in samples[span_start..span_end]
            .iter()
            .copied()
            .filter(|(_, _, sample)| close_to_nearest(sample))
        {
            if let Some(previous) = previous_parameter {
                let scale = sample.total_parameter.abs().max(previous.abs()).max(1.0);
                if (sample.total_parameter - previous).abs() > 64.0 * f64::EPSILON * scale {
                    branch_ordinal = branch_ordinal.saturating_add(1);
                }
            }
            previous_parameter = Some(sample.total_parameter);
            let Some(contact) = draft_curve_contact(
                document,
                curve.span,
                curve.contact_domain,
                sample.total_parameter,
            ) else {
                continue;
            };
            anchors.push(DraftReferenceAnchor::CurvePoint {
                contact,
                branch_candidate: DraftCurveBranchCandidate::from_ordinal(branch_ordinal),
                model_position: sample.model_position,
                role: curve.role,
                source_role: curve.source_role,
                origin,
            });
        }
        span_start = span_end;
    }
    Ok(())
}

fn scene_curve_pointer_samples(
    curve: &SceneCurve,
    document: &SketchDocument,
    viewport: Viewport,
    pointer: ScreenPoint,
) -> Vec<SceneCurvePointerSample> {
    let mut projections = curve
        .screen_polyline
        .windows(2)
        .zip(curve.screen_parameters.windows(2))
        .enumerate()
        .filter_map(|(segment_index, (segment, parameters))| {
            let (distance, projection) = point_segment_projection(pointer, segment[0], segment[1]);
            let parameter = (parameters[1] - parameters[0]).mul_add(projection, parameters[0]);
            (distance.is_finite() && parameter.is_finite()).then_some(SceneCurveChordProjection {
                segment_index,
                distance_pixels: distance,
                parameter,
                lower_parameter: parameters[0].min(parameters[1]),
                upper_parameter: parameters[0].max(parameters[1]),
            })
        })
        .collect::<Vec<_>>();
    let Some(nearest_distance) = projections
        .iter()
        .map(|projection| projection.distance_pixels)
        .min_by(f64::total_cmp)
    else {
        return Vec::new();
    };
    projections.retain(|projection| {
        projection.distance_pixels <= nearest_distance + CURVE_BRANCH_CANDIDATE_BAND_PIXELS
    });

    let mut samples = Vec::new();
    let mut group_start = 0;
    for index in 1..=projections.len() {
        let continues_group = index < projections.len()
            && projections[index].segment_index == projections[index - 1].segment_index + 1;
        if continues_group {
            continue;
        }
        if let Some(sample) = refine_scene_curve_pointer_sample(
            curve.span,
            document,
            viewport,
            pointer,
            &projections[group_start..index],
        ) {
            samples.push(sample);
        }
        group_start = index;
    }
    let Some(best_exact_distance) = samples
        .iter()
        .map(|sample| sample.distance_pixels)
        .min_by(f64::total_cmp)
    else {
        return Vec::new();
    };
    samples.retain(|sample| {
        sample.distance_pixels <= best_exact_distance + CURVE_BRANCH_CANDIDATE_BAND_PIXELS
    });
    samples.sort_by(|first, second| {
        first
            .total_parameter
            .total_cmp(&second.total_parameter)
            .then_with(|| first.distance_pixels.total_cmp(&second.distance_pixels))
    });
    samples.dedup_by(|first, second| {
        let scale = first
            .total_parameter
            .abs()
            .max(second.total_parameter.abs())
            .max(1.0);
        (first.total_parameter - second.total_parameter).abs() <= 64.0 * f64::EPSILON * scale
    });
    samples
}

fn refine_scene_curve_pointer_sample(
    span: CurveSpan,
    document: &SketchDocument,
    viewport: Viewport,
    pointer: ScreenPoint,
    projections: &[SceneCurveChordProjection],
) -> Option<SceneCurvePointerSample> {
    let seed = projections.iter().min_by(|first, second| {
        first
            .distance_pixels
            .total_cmp(&second.distance_pixels)
            .then_with(|| first.parameter.total_cmp(&second.parameter))
    })?;
    let lower = projections
        .iter()
        .map(|projection| projection.lower_parameter)
        .min_by(f64::total_cmp)?;
    let upper = projections
        .iter()
        .map(|projection| projection.upper_parameter)
        .max_by(f64::total_cmp)?;
    if !lower.is_finite() || !upper.is_finite() || lower >= upper {
        return None;
    }
    let pointer_model = viewport.screen_to_model(pointer);
    let mut parameter = seed.parameter.clamp(lower, upper);
    for _ in 0..CURVE_POINTER_REFINEMENT_STEPS {
        let jet = document.evaluate_curve_jet(span, parameter).ok()?;
        let residual = [
            jet.position.x - pointer_model[0],
            jet.position.y - pointer_model[1],
        ];
        let first = [jet.first_derivative.x, jet.first_derivative.y];
        let second = [jet.second_derivative.x, jet.second_derivative.y];
        let stationarity = residual[0].mul_add(first[0], residual[1] * first[1]);
        let derivative = first[0].mul_add(
            first[0],
            first[1] * first[1] + residual[0].mul_add(second[0], residual[1] * second[1]),
        );
        if !stationarity.is_finite() || !derivative.is_finite() || derivative.abs() <= f64::EPSILON
        {
            break;
        }
        let next = (parameter - stationarity / derivative).clamp(lower, upper);
        if !next.is_finite() || next.to_bits() == parameter.to_bits() {
            break;
        }
        parameter = next;
    }
    let model_position = exact_curve_model_position(document, span, parameter)?;
    let distance_pixels = viewport.model_to_screen(model_position).distance(pointer);
    distance_pixels
        .is_finite()
        .then_some(SceneCurvePointerSample {
            total_parameter: parameter,
            model_position,
            distance_pixels,
        })
}

fn scene_curve_model_position_at_parameter(
    curve: &SceneCurve,
    document: &SketchDocument,
    parameter: f64,
) -> Option<[f64; 2]> {
    let occurs_on_painted_interval = curve
        .screen_polyline
        .windows(2)
        .zip(curve.screen_parameters.windows(2))
        .any(|(_, parameters)| {
            let lower = parameters[0].min(parameters[1]);
            let upper = parameters[0].max(parameters[1]);
            (lower..=upper).contains(&parameter)
                && parameters[0].to_bits() != parameters[1].to_bits()
        });
    occurs_on_painted_interval
        .then(|| exact_curve_model_position(document, curve.span, parameter))
        .flatten()
}

fn exact_curve_model_position(
    document: &SketchDocument,
    span: CurveSpan,
    parameter: f64,
) -> Option<[f64; 2]> {
    let position = document.evaluate_curve_jet(span, parameter).ok()?.position;
    let model_position = [position.x, position.y];
    model_position
        .into_iter()
        .all(f64::is_finite)
        .then_some(model_position)
}

fn scene_curve_affine_direction(curve: &SceneCurve, viewport: Viewport) -> Option<[f64; 2]> {
    let first_parameter = *curve.screen_parameters.first()?;
    let last_parameter = *curve.screen_parameters.last()?;
    let first = viewport.screen_to_model(*curve.screen_polyline.first()?);
    let last = viewport.screen_to_model(*curve.screen_polyline.last()?);
    let direction = if first_parameter <= last_parameter {
        [last[0] - first[0], last[1] - first[1]]
    } else {
        [first[0] - last[0], first[1] - last[1]]
    };
    let length = direction[0].hypot(direction[1]);
    (length.is_finite() && length > 0.0).then_some([direction[0] / length, direction[1] / length])
}

fn draft_curve_contact(
    document: &SketchDocument,
    span: CurveSpan,
    domain: ContactDomain,
    total_parameter: f64,
) -> Option<DraftCurveContact> {
    if !total_parameter.is_finite() {
        return None;
    }
    let neighborhood = document
        .picked_contact_neighborhood(span, total_parameter)
        .ok()?;
    let (parameter, winding) = match domain {
        ContactDomain::SupportingLine => (total_parameter, 0),
        ContactDomain::Bounded { lower, upper }
            if lower.is_finite()
                && upper.is_finite()
                && lower < upper
                && (lower..=upper).contains(&total_parameter) =>
        {
            (total_parameter, 0)
        }
        ContactDomain::Periodic { period } if period.is_finite() && period > 0.0 => {
            let principal = total_parameter.rem_euclid(period);
            let winding = periodic_contact_winding(total_parameter, principal, period)?;
            (principal, winding)
        }
        ContactDomain::Bounded { .. } | ContactDomain::Periodic { .. } => return None,
    };
    Some(DraftCurveContact {
        span,
        domain,
        parameter,
        winding,
        neighborhood,
    })
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "the finite quotient is range-checked against i32 before conversion"
)]
fn periodic_contact_winding(total: f64, principal: f64, period: f64) -> Option<i32> {
    let winding = ((total - principal) / period).round();
    (winding.is_finite() && winding >= f64::from(i32::MIN) && winding <= f64::from(i32::MAX))
        .then_some(winding as i32)
}

fn curve_hit(curve: &SceneCurve, position: ScreenPoint, tolerance_pixels: f64) -> Option<Hit> {
    let (distance, parameter) = curve
        .screen_polyline
        .windows(2)
        .zip(curve.screen_parameters.windows(2))
        .map(|(segment, parameters)| {
            let (distance, projection) = point_segment_projection(position, segment[0], segment[1]);
            (
                distance,
                (parameters[1] - parameters[0]).mul_add(projection, parameters[0]),
            )
        })
        .min_by(|first, second| first.0.total_cmp(&second.0))?;
    (distance <= tolerance_pixels).then_some(Hit {
        item: SelectionItem::Curve(curve.span),
        distance_pixels: distance,
        curve_parameter: Some(parameter),
        geometry: Some(SceneGeometryHit::NativeCurve {
            role: curve.role,
            source_role: curve.source_role,
            origin: curve.origin,
        }),
    })
}

fn computed_curve_hit(
    curve: &SceneComputedCurve,
    position: ScreenPoint,
    tolerance_pixels: f64,
) -> Option<Hit> {
    let distance = curve
        .screen_polyline
        .windows(2)
        .map(|segment| point_segment_projection(position, segment[0], segment[1]).0)
        .min_by(f64::total_cmp)?;
    (distance <= tolerance_pixels).then_some(Hit {
        item: SelectionItem::FeatureCorner(curve.owner),
        distance_pixels: distance,
        curve_parameter: None,
        geometry: Some(SceneGeometryHit::ComputedFilletArc {
            edge: curve.edge,
            owner: curve.owner,
            role: curve.role,
        }),
    })
}

fn document_contains_item(document: &SketchDocument, item: SelectionItem) -> bool {
    match item {
        SelectionItem::Point(point) => document.point(point).is_some(),
        SelectionItem::Curve(span) => document
            .curve_spans(span.curve)
            .is_ok_and(|spans| spans.contains(&span)),
        SelectionItem::Constraint(_)
        | SelectionItem::Dimension(_)
        | SelectionItem::Datum(_)
        | SelectionItem::Feature(_)
        | SelectionItem::FeatureCorner(_) => false,
    }
}

#[derive(Default)]
struct PolicyHitAccumulator {
    profile_nearest: Option<f64>,
    profile_best: Option<Hit>,
    construction_nearest: Option<f64>,
    construction_best: Option<Hit>,
}

impl PolicyHitAccumulator {
    fn consider(&mut self, hit: Hit, scope: GeometryPickScope) {
        let role = hit.geometry.map_or(GeometryRole::Profile, |geometry| {
            geometry.preferred_role(scope)
        });
        let (nearest, best) = match role {
            GeometryRole::Profile => (&mut self.profile_nearest, &mut self.profile_best),
            GeometryRole::Construction => {
                (&mut self.construction_nearest, &mut self.construction_best)
            }
        };
        if nearest.is_none_or(|distance| hit.distance_pixels < distance) {
            *nearest = Some(hit.distance_pixels);
        }
        if best
            .as_ref()
            .is_none_or(|candidate| compare_same_role_hits(&hit, candidate).is_lt())
        {
            *best = Some(hit);
        }
    }

    fn best(&self, scope: GeometryPickScope) -> Option<Hit> {
        match scope {
            GeometryPickScope::Profile => self.profile_best,
            GeometryPickScope::Construction => self.construction_best,
            GeometryPickScope::All => match (
                self.profile_nearest,
                self.profile_best,
                self.construction_nearest,
                self.construction_best,
            ) {
                (
                    Some(profile_distance),
                    Some(profile),
                    Some(construction_distance),
                    Some(construction),
                ) => {
                    if profile_distance <= construction_distance + 1.0 {
                        Some(profile)
                    } else {
                        Some(construction)
                    }
                }
                (Some(_), Some(profile), _, _) => Some(profile),
                (_, _, Some(_), Some(construction)) => Some(construction),
                _ => None,
            },
        }
    }
}

fn best_policy_hit(hits: impl IntoIterator<Item = Hit>, scope: GeometryPickScope) -> Option<Hit> {
    let mut candidates = PolicyHitAccumulator::default();
    for hit in hits {
        candidates.consider(hit, scope);
    }
    candidates.best(scope)
}

fn compare_same_role_hits(first: &Hit, second: &Hit) -> Ordering {
    native_hit_priority(first.item)
        .cmp(&native_hit_priority(second.item))
        .then_with(|| first.distance_pixels.total_cmp(&second.distance_pixels))
        .then_with(|| first.item.cmp(&second.item))
        .then_with(|| match (first.curve_parameter, second.curve_parameter) {
            (Some(first), Some(second)) => first.total_cmp(&second),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        })
}

const fn native_hit_priority(item: SelectionItem) -> u8 {
    match item {
        SelectionItem::Point(_) => 0,
        SelectionItem::Curve(_) => 1,
        SelectionItem::Constraint(_)
        | SelectionItem::Dimension(_)
        | SelectionItem::Datum(_)
        | SelectionItem::Feature(_)
        | SelectionItem::FeatureCorner(_) => 2,
    }
}

fn item_belongs_to_computed_feature(
    item: SelectionItem,
    feature: geosolve_sketch_features::ComputedFeatureId,
) -> bool {
    match item {
        SelectionItem::Feature(candidate) => candidate == feature,
        SelectionItem::FeatureCorner(owner) => owner.feature == feature,
        SelectionItem::Point(_)
        | SelectionItem::Curve(_)
        | SelectionItem::Constraint(_)
        | SelectionItem::Dimension(_)
        | SelectionItem::Datum(_) => false,
    }
}

fn point_segment_projection(
    point: ScreenPoint,
    start: ScreenPoint,
    end: ScreenPoint,
) -> (f64, f64) {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length_squared = dx.mul_add(dx, dy * dy);
    if length_squared == 0.0 {
        return (point.distance(start), 0.0);
    }
    let projection = ((point.x - start.x).mul_add(dx, (point.y - start.y) * dy) / length_squared)
        .clamp(0.0, 1.0);
    (
        point.distance(ScreenPoint {
            x: dx.mul_add(projection, start.x),
            y: dy.mul_add(projection, start.y),
        }),
        projection,
    )
}

fn screen_points_match(first: ScreenPoint, second: ScreenPoint) -> bool {
    let scale = first
        .x
        .abs()
        .max(first.y.abs())
        .max(second.x.abs())
        .max(second.y.abs())
        .max(1.0);
    first.distance(second) <= 1.0e-10 * scale
}

fn model_point_segment_distance(point: [f64; 2], start: [f64; 2], end: [f64; 2]) -> f64 {
    let delta = [end[0] - start[0], end[1] - start[1]];
    let length_squared = delta[0].mul_add(delta[0], delta[1].powi(2));
    if !length_squared.is_finite() || length_squared <= 0.0 {
        return (point[0] - start[0]).hypot(point[1] - start[1]);
    }
    let projection = ((point[0] - start[0]).mul_add(delta[0], (point[1] - start[1]) * delta[1])
        / length_squared)
        .clamp(0.0, 1.0);
    let projected = [
        delta[0].mul_add(projection, start[0]),
        delta[1].mul_add(projection, start[1]),
    ];
    (point[0] - projected[0]).hypot(point[1] - projected[1])
}

fn action_model_hit_distance(
    action: &SceneFilletAction,
    model_position: [f64; 2],
    viewport: Viewport,
    tolerance: PickTolerance,
) -> Option<f64> {
    let mut best = None::<f64>;
    if let Some(control) = action.control_geometry {
        let direction_length = control.model_direction[0].hypot(control.model_direction[1]);
        let model_length =
            control.screen_start.distance(control.screen_end) / viewport.pixels_per_model_unit;
        if direction_length.is_finite()
            && direction_length > 0.0
            && model_length.is_finite()
            && model_length > 0.0
        {
            let model_end = [
                (model_length * control.model_direction[0] / direction_length)
                    .mul_add(1.0, control.model_anchor[0]),
                (model_length * control.model_direction[1] / direction_length)
                    .mul_add(1.0, control.model_anchor[1]),
            ];
            let segment_pixels =
                model_point_segment_distance(model_position, control.model_anchor, model_end)
                    * viewport.pixels_per_model_unit;
            let endpoint_pixels = (model_position[0] - model_end[0])
                .hypot(model_position[1] - model_end[1])
                * viewport.pixels_per_model_unit;
            if segment_pixels <= tolerance.curve_pixels {
                best = Some(segment_pixels);
            }
            if endpoint_pixels <= tolerance.point_pixels {
                best = Some(best.map_or(endpoint_pixels, |current| current.min(endpoint_pixels)));
            }
        }
    }
    if let Some(geometry) = &action.dashed_alternative_arc {
        let distance = geometry
            .model_polyline
            .windows(2)
            .map(|pair| model_point_segment_distance(model_position, pair[0], pair[1]))
            .min_by(f64::total_cmp)?
            * viewport.pixels_per_model_unit;
        if distance <= tolerance.curve_pixels {
            best = Some(best.map_or(distance, |current| current.min(distance)));
        }
    }
    best
}

const fn fillet_action_order_key(action: SceneFilletActionId) -> (u8, u8, u8) {
    match action {
        SceneFilletActionId::ReverseFirstRetainedDirection => (0, 0, 0),
        SceneFilletActionId::ReverseSecondRetainedDirection => (1, 0, 0),
        SceneFilletActionId::ComplementaryArc => (2, 0, 0),
        SceneFilletActionId::LocalAlternative { first, second } => (
            3,
            fillet_normal_side_order(first),
            fillet_normal_side_order(second),
        ),
    }
}

const fn fillet_normal_side_order(side: DocumentCurveNormalSide) -> u8 {
    match side {
        DocumentCurveNormalSide::Left => 0,
        DocumentCurveNormalSide::Right => 1,
    }
}

fn polyline_proposal(draft: &Draft) -> Option<ConstructionProposal> {
    (draft.points.len() >= 2 && draft.positions.windows(2).all(nonzero_segment)).then(|| {
        if draft.exact_variant {
            ConstructionProposal::PolylinePath {
                points: draft.points.clone(),
                closed: draft.closed,
            }
        } else {
            ConstructionProposal::Polyline {
                points: draft.points.clone(),
            }
        }
    })
}

fn nurbs_proposal(draft: &Draft) -> Option<ConstructionProposal> {
    let mut options = draft.nurbs_options.clone();
    if draft.exact_variant {
        options.form = match draft.variant {
            GeometryToolVariant::PeriodicControlNurbs => DocumentBSplineForm::Periodic,
            _ => DocumentBSplineForm::Clamped,
        };
    }
    let minimum = usize::try_from(options.degree).ok()?.checked_add(1)?;
    (draft.points.len() >= minimum
        && draft.positions.windows(2).all(nonzero_segment)
        && validate_nurbs_for_controls(&options, draft.points.len()).is_ok())
    .then(|| ConstructionProposal::Nurbs {
        controls: draft.points.clone(),
        options,
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct EllipticalArcClickProjection {
    parameter: f64,
    position: [f64; 2],
}

/// Radially projects a spatial trim click through normalized ellipse space.
///
/// The parameter convention matches `Ellipse2`: zero is the stored positive
/// major direction and positive angles turn toward its left-normal minor axis.
/// The resulting point is therefore on the exact support ellipse while the
/// raw click remains transient interaction input.
fn project_elliptical_arc_click(
    center: [f64; 2],
    major_axis_point: [f64; 2],
    minor_axis_ratio: f64,
    target: [f64; 2],
) -> Option<EllipticalArcClickProjection> {
    if !center.into_iter().all(f64::is_finite)
        || !major_axis_point.into_iter().all(f64::is_finite)
        || !target.into_iter().all(f64::is_finite)
        || !minor_axis_ratio.is_finite()
        || minor_axis_ratio <= 0.0
        || minor_axis_ratio > 1.0
    {
        return None;
    }
    let major = [
        major_axis_point[0] - center[0],
        major_axis_point[1] - center[1],
    ];
    let semi_major = major[0].hypot(major[1]);
    let semi_minor = semi_major * minor_axis_ratio;
    if !semi_major.is_finite() || !semi_minor.is_finite() || semi_major <= 0.0 || semi_minor <= 0.0
    {
        return None;
    }
    let major_direction = [major[0] / semi_major, major[1] / semi_major];
    let minor_direction = [-major_direction[1], major_direction[0]];
    let difference = [target[0] - center[0], target[1] - center[1]];
    let normalized_major =
        difference[0].mul_add(major_direction[0], difference[1] * major_direction[1]) / semi_major;
    let normalized_minor =
        difference[0].mul_add(minor_direction[0], difference[1] * minor_direction[1]) / semi_minor;
    if !normalized_major.is_finite()
        || !normalized_minor.is_finite()
        || (normalized_major == 0.0 && normalized_minor == 0.0)
    {
        return None;
    }
    let parameter = normalized_minor.atan2(normalized_major);
    let major_offset = semi_major * parameter.cos();
    let minor_offset = semi_minor * parameter.sin();
    let position = [
        major_offset.mul_add(
            major_direction[0],
            minor_offset.mul_add(minor_direction[0], center[0]),
        ),
        major_offset.mul_add(
            major_direction[1],
            minor_offset.mul_add(minor_direction[1], center[1]),
        ),
    ];
    (parameter.is_finite() && position.into_iter().all(f64::is_finite)).then_some(
        EllipticalArcClickProjection {
            parameter,
            position,
        },
    )
}

fn elliptical_arc_click_projection(
    draft: &Draft,
    index: usize,
) -> Option<EllipticalArcClickProjection> {
    project_elliptical_arc_click(
        *draft.positions.first()?,
        *draft.positions.get(1)?,
        draft.conic_options.minor_axis_ratio,
        *draft.positions.get(index)?,
    )
}

fn legacy_elliptical_arc_preview_positions(draft: &Draft) -> Option<Vec<[f64; 2]>> {
    if draft.tool != EditorTool::EllipticalArc || draft.positions.len() > 4 {
        return None;
    }
    let mut positions = draft.positions.clone();
    for (index, position) in positions.iter_mut().enumerate().skip(2) {
        *position = elliptical_arc_click_projection(draft, index)?.position;
    }
    Some(positions)
}

fn legacy_elliptical_arc_support_preview(draft: &Draft) -> Option<ConstructionPreview> {
    let [center, major_axis_point, ..] = draft.positions.as_slice() else {
        return None;
    };
    let positions = legacy_elliptical_arc_preview_positions(draft)?;
    let proposal = ConstructionProposal::Ellipse {
        center: draft.points[0],
        major_axis_point: draft.points[1],
        minor_axis_ratio: draft.conic_options.minor_axis_ratio,
    };
    let ConstructionPreviewGeometry::AdvancedCurve { curve_points, .. } =
        advanced_curve_preview(&proposal, &positions[..2], EditorTool::Ellipse)?
    else {
        return None;
    };
    Some(ConstructionPreview::EllipticalArcSupport {
        center: *center,
        major_axis_point: *major_axis_point,
        support_points: curve_points,
        trim_start: positions.get(2).copied(),
    })
}

fn elliptical_arc_preview_positions(draft: &Draft) -> Option<Vec<[f64; 2]>> {
    if !matches!(
        draft.variant,
        GeometryToolVariant::CenterAxesEllipticalArc
            | GeometryToolVariant::AxisEndpointsEllipticalArc
    ) || draft.positions.len() > 5
    {
        return None;
    }
    let frame = ellipse_recipe_frame(draft)?;
    let mut positions = draft.positions.clone();
    for position in positions.iter_mut().skip(frame.trim_start_stage) {
        *position = ellipse_project_sample(frame, *position)?.1;
    }
    Some(positions)
}

fn elliptical_arc_support_preview(draft: &Draft) -> Option<ConstructionPreview> {
    let frame = ellipse_recipe_frame(draft)?;
    let positions = elliptical_arc_preview_positions(draft)?;
    let (proposal, controls) = match draft.variant {
        GeometryToolVariant::CenterAxesEllipticalArc => (
            ConstructionProposal::Ellipse {
                center: draft.points[0],
                major_axis_point: draft.points[1],
                minor_axis_ratio: frame.ratio,
            },
            vec![frame.center, frame.major_axis_point],
        ),
        GeometryToolVariant::AxisEndpointsEllipticalArc => (
            ConstructionProposal::AxisEndpointEllipse {
                major_axis_point: draft.points[0],
                center: ConstructionPoint::New(frame.center),
                minor_axis_ratio: frame.ratio,
            },
            vec![frame.major_axis_point, frame.center],
        ),
        _ => return None,
    };
    let ConstructionPreviewGeometry::AdvancedCurve { curve_points, .. } =
        advanced_curve_preview(&proposal, &controls, EditorTool::Ellipse)?
    else {
        return None;
    };
    Some(ConstructionPreview::EllipticalArcSupport {
        center: frame.center,
        major_axis_point: frame.major_axis_point,
        support_points: curve_points,
        trim_start: positions.get(frame.trim_start_stage).copied(),
    })
}

fn elliptical_arc_sweep_is_nonzero(start: f64, end: f64, sweep: DocumentArcSweep) -> bool {
    let magnitude = match sweep {
        DocumentArcSweep::CounterClockwise => (end - start).rem_euclid(std::f64::consts::TAU),
        DocumentArcSweep::Clockwise => (start - end).rem_euclid(std::f64::consts::TAU),
    };
    magnitude.is_finite() && magnitude > 0.0
}

fn valid_draft_stage(draft: &Draft) -> bool {
    if !draft.exact_variant {
        return legacy_valid_draft_stage(draft);
    }
    match draft.variant {
        GeometryToolVariant::Polyline
        | GeometryToolVariant::OpenControlNurbs
        | GeometryToolVariant::PeriodicControlNurbs => {
            draft.positions.windows(2).all(nonzero_segment)
        }
        variant => geometry_variant_required_stages(variant).is_some_and(|required| {
            draft.positions.len() < required
                || (draft.positions.len() == required && draft_proposal(draft).is_some())
        }),
    }
}

fn legacy_valid_draft_stage(draft: &Draft) -> bool {
    match draft.tool {
        EditorTool::Point => draft.positions.len() == 1,
        EditorTool::Line
        | EditorTool::Rectangle
        | EditorTool::Circle
        | EditorTool::Ellipse
        | EditorTool::Parabola
        | EditorTool::Hyperbola => draft.positions.len() < 2 || draft_proposal(draft).is_some(),
        EditorTool::Polyline | EditorTool::Nurbs => draft.positions.windows(2).all(nonzero_segment),
        EditorTool::CounterClockwiseArc => {
            let start_is_valid =
                draft.positions.len() < 2 || nonzero_segment(&draft.positions[..2]);
            start_is_valid && (draft.positions.len() < 3 || draft_proposal(draft).is_some())
        }
        EditorTool::QuadraticBezier | EditorTool::RationalQuadraticConic => {
            draft.positions.len() < 3 || draft_proposal(draft).is_some()
        }
        EditorTool::CubicBezier => draft.positions.len() < 4 || draft_proposal(draft).is_some(),
        EditorTool::EllipticalArc => {
            let axis_is_valid = draft.positions.len() < 2 || nonzero_segment(&draft.positions[..2]);
            let start_is_valid =
                draft.positions.len() < 3 || elliptical_arc_click_projection(draft, 2).is_some();
            axis_is_valid
                && start_is_valid
                && (draft.positions.len() < 4 || draft_proposal(draft).is_some())
        }
        EditorTool::Select => false,
    }
}

const fn geometry_variant_required_stages(variant: GeometryToolVariant) -> Option<usize> {
    use GeometryToolVariant as V;
    Some(match variant {
        V::SketchPoint => 1,
        V::Segment
        | V::MidpointLine
        | V::TwoPointAlignedRectangle
        | V::CenterRectangle
        | V::CenterRadiusCircle
        | V::TwoPointDiameterCircle
        | V::TangentArc
        | V::Parabola
        | V::Hyperbola => 2,
        V::ThreePointCornerRectangle
        | V::ThreePointCenterRectangle
        | V::ThreePointCircle
        | V::CenterArc
        | V::ThreePointArc
        | V::CenterAxesEllipse
        | V::AxisEndpointsEllipse
        | V::QuadraticBezier
        | V::RationalQuadraticConic => 3,
        V::CubicBezier => 4,
        V::CenterAxesEllipticalArc | V::AxisEndpointsEllipticalArc => 5,
        V::Polyline | V::OpenControlNurbs | V::PeriodicControlNurbs => return None,
    })
}

fn geometry_draft_stage(
    variant: GeometryToolVariant,
    completed: usize,
) -> Option<GeometryDraftStage> {
    use GeometryDraftStage as S;
    use GeometryToolVariant as V;

    Some(match variant {
        V::SketchPoint => (completed == 0).then_some(S::Point)?,
        V::Segment => [S::Start, S::End].get(completed).copied()?,
        V::Polyline => {
            if completed == 0 {
                S::Start
            } else {
                S::End
            }
        }
        V::MidpointLine | V::CenterRadiusCircle => [S::Center, S::End].get(completed).copied()?,
        V::TwoPointAlignedRectangle => [S::Corner, S::OppositeCorner].get(completed).copied()?,
        V::ThreePointCornerRectangle => [S::Corner, S::AdjacentCorner, S::AdjacentCorner]
            .get(completed)
            .copied()?,
        V::CenterRectangle => [S::Center, S::Corner].get(completed).copied()?,
        V::ThreePointCenterRectangle => [S::Center, S::SideMidpoint, S::Corner]
            .get(completed)
            .copied()?,
        V::TwoPointDiameterCircle => [S::DiameterStart, S::DiameterEnd].get(completed).copied()?,
        V::ThreePointCircle | V::ThreePointArc => [S::Start, S::End, S::ThroughPoint]
            .get(completed)
            .copied()?,
        V::CenterArc => [S::Center, S::Start, S::End].get(completed).copied()?,
        V::TangentArc => [S::SourceEndpoint, S::End].get(completed).copied()?,
        V::CenterAxesEllipse => [S::Center, S::MajorAxisEndpoint, S::MinorExtent]
            .get(completed)
            .copied()?,
        V::AxisEndpointsEllipse => [
            S::MajorAxisEndpoint,
            S::OppositeAxisEndpoint,
            S::MinorExtent,
        ]
        .get(completed)
        .copied()?,
        V::CenterAxesEllipticalArc => [
            S::Center,
            S::MajorAxisEndpoint,
            S::MinorExtent,
            S::TrimStart,
            S::TrimEnd,
        ]
        .get(completed)
        .copied()?,
        V::AxisEndpointsEllipticalArc => [
            S::MajorAxisEndpoint,
            S::OppositeAxisEndpoint,
            S::MinorExtent,
            S::TrimStart,
            S::TrimEnd,
        ]
        .get(completed)
        .copied()?,
        V::QuadraticBezier | V::RationalQuadraticConic => [S::Start, S::ControlPoint, S::End]
            .get(completed)
            .copied()?,
        V::CubicBezier => [S::Start, S::ControlPoint, S::ControlPoint, S::End]
            .get(completed)
            .copied()?,
        V::Parabola => [S::Vertex, S::Focus].get(completed).copied()?,
        V::Hyperbola => [S::Center, S::TransverseAxisEndpoint]
            .get(completed)
            .copied()?,
        V::OpenControlNurbs | V::PeriodicControlNurbs => S::ControlPoint,
    })
}

fn geometry_draft_measurements(draft: &Draft) -> Vec<GeometryDraftMeasurement> {
    let mut measurements = Vec::new();
    if let Some([start, end]) = draft
        .positions
        .windows(2)
        .last()
        .map(|pair| [pair[0], pair[1]])
    {
        let delta = [end[0] - start[0], end[1] - start[1]];
        let length = delta[0].hypot(delta[1]);
        if length.is_finite() {
            measurements.push(GeometryDraftMeasurement::Length(length));
            measurements.push(GeometryDraftMeasurement::AngleRadians(
                delta[1].atan2(delta[0]),
            ));
        }
    }
    if let Some(proposal) = draft_proposal(draft) {
        match proposal {
            ConstructionProposal::Circle { radius, .. } => {
                measurements.push(GeometryDraftMeasurement::Radius(radius));
                measurements.push(GeometryDraftMeasurement::Diameter(2.0 * radius));
            }
            ConstructionProposal::CircularArc {
                center,
                start,
                end,
                sweep,
            } => {
                let center = center.position();
                let radius = (start[0] - center[0]).hypot(start[1] - center[1]);
                let start_angle = (start[1] - center[1]).atan2(start[0] - center[0]);
                let end_angle = (end[1] - center[1]).atan2(end[0] - center[0]);
                let angle = match sweep {
                    DocumentArcSweep::CounterClockwise => {
                        (end_angle - start_angle).rem_euclid(std::f64::consts::TAU)
                    }
                    DocumentArcSweep::Clockwise => {
                        (start_angle - end_angle).rem_euclid(std::f64::consts::TAU)
                    }
                };
                measurements.push(GeometryDraftMeasurement::Radius(radius));
                measurements.push(GeometryDraftMeasurement::AngleRadians(angle));
            }
            ConstructionProposal::CounterClockwiseArc { center, start, end } => {
                let center = center.position();
                let radius = (start[0] - center[0]).hypot(start[1] - center[1]);
                let start_angle = (start[1] - center[1]).atan2(start[0] - center[0]);
                let end_angle = (end[1] - center[1]).atan2(end[0] - center[0]);
                let angle = (end_angle - start_angle).rem_euclid(std::f64::consts::TAU);
                measurements.push(GeometryDraftMeasurement::Radius(radius));
                measurements.push(GeometryDraftMeasurement::AngleRadians(angle));
            }
            ConstructionProposal::RectangleLoop {
                ref points,
                corners,
                ..
            } => {
                let first = points[corners[0]].position();
                let second = points[corners[1]].position();
                let third = points[corners[2]].position();
                measurements.push(GeometryDraftMeasurement::WidthHeight {
                    width: (second[0] - first[0]).hypot(second[1] - first[1]),
                    height: (third[0] - second[0]).hypot(third[1] - second[1]),
                });
            }
            ConstructionProposal::Ellipse {
                minor_axis_ratio, ..
            }
            | ConstructionProposal::AxisEndpointEllipse {
                minor_axis_ratio, ..
            }
            | ConstructionProposal::EllipticalArc {
                minor_axis_ratio, ..
            }
            | ConstructionProposal::AxisEndpointEllipticalArc {
                minor_axis_ratio, ..
            } => measurements.push(GeometryDraftMeasurement::Ratio(minor_axis_ratio)),
            _ => {}
        }
    }
    if matches!(
        draft.variant,
        GeometryToolVariant::Polyline
            | GeometryToolVariant::OpenControlNurbs
            | GeometryToolVariant::PeriodicControlNurbs
    ) {
        measurements.push(GeometryDraftMeasurement::ControlCount(draft.points.len()));
    }
    measurements
}

fn nonzero_segment(segment: &[[f64; 2]]) -> bool {
    let [start, end] = segment else {
        return false;
    };
    let length = (end[0] - start[0]).hypot(end[1] - start[1]);
    length.is_finite() && length > 0.0
}

fn construction_point_at(original: ConstructionPoint, position: [f64; 2]) -> ConstructionPoint {
    match original {
        ConstructionPoint::Existing {
            id,
            position: existing,
        } if model_positions_bit_equal(existing, position) => ConstructionPoint::Existing {
            id,
            position: existing,
        },
        ConstructionPoint::Existing { .. } | ConstructionPoint::New(_) => {
            ConstructionPoint::New(position)
        }
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive rectangle recipe table keeps derived corners and stored identity auditable"
)]
fn rectangle_loop_proposal(draft: &Draft) -> Option<ConstructionProposal> {
    use GeometryToolVariant as V;

    match draft.variant {
        V::TwoPointAlignedRectangle if draft.positions.len() == 2 => {
            let first = draft.positions[0];
            let mut opposite = draft.positions[1];
            let delta = [opposite[0] - first[0], opposite[1] - first[1]];
            if draft.regularized {
                let side = delta[0].abs().max(delta[1].abs());
                if !(side.is_finite() && side > 0.0) {
                    return None;
                }
                opposite = [
                    first[0] + side.copysign(delta[0]),
                    first[1] + side.copysign(delta[1]),
                ];
            }
            if !nonzero_segment(&[first, [opposite[0], first[1]]])
                || !nonzero_segment(&[[opposite[0], first[1]], opposite])
            {
                return None;
            }
            let first_operand = construction_point_at(draft.points[0], first);
            let opposite_operand = construction_point_at(draft.points[1], opposite);
            Some(ConstructionProposal::RectangleLoop {
                points: vec![
                    first_operand,
                    opposite_operand,
                    ConstructionPoint::New([opposite[0], first[1]]),
                    ConstructionPoint::New([first[0], opposite[1]]),
                ],
                corners: [0, 2, 1, 3],
                center: None,
            })
        }
        V::ThreePointCornerRectangle if draft.positions.len() == 3 => {
            let first = draft.positions[0];
            let adjacent = draft.positions[1];
            let axis = [adjacent[0] - first[0], adjacent[1] - first[1]];
            let length = axis[0].hypot(axis[1]);
            if !(length.is_finite() && length > 0.0) {
                return None;
            }
            let normal = [-axis[1] / length, axis[0] / length];
            let raw = draft.positions[2];
            let mut height =
                (raw[0] - adjacent[0]).mul_add(normal[0], (raw[1] - adjacent[1]) * normal[1]);
            if draft.regularized {
                let sign = if height.is_sign_negative() { -1.0 } else { 1.0 };
                height = sign * length;
            }
            if !height.is_finite() || height == 0.0 {
                return None;
            }
            let third = [
                adjacent[0] + normal[0] * height,
                adjacent[1] + normal[1] * height,
            ];
            let fourth = [first[0] + normal[0] * height, first[1] + normal[1] * height];
            Some(ConstructionProposal::RectangleLoop {
                points: vec![
                    construction_point_at(draft.points[0], first),
                    construction_point_at(draft.points[1], adjacent),
                    construction_point_at(draft.points[2], third),
                    ConstructionPoint::New(fourth),
                ],
                corners: [0, 1, 2, 3],
                center: None,
            })
        }
        V::CenterRectangle if draft.positions.len() == 2 => {
            let center = draft.positions[0];
            let mut corner = draft.positions[1];
            let delta = [corner[0] - center[0], corner[1] - center[1]];
            if draft.regularized {
                let half = delta[0].abs().max(delta[1].abs());
                if !(half.is_finite() && half > 0.0) {
                    return None;
                }
                corner = [
                    center[0] + half.copysign(delta[0]),
                    center[1] + half.copysign(delta[1]),
                ];
            }
            let opposite = [2.0 * center[0] - corner[0], 2.0 * center[1] - corner[1]];
            let second = [opposite[0], corner[1]];
            let fourth = [corner[0], opposite[1]];
            if !nonzero_segment(&[corner, second]) || !nonzero_segment(&[second, opposite]) {
                return None;
            }
            Some(ConstructionProposal::RectangleLoop {
                points: vec![
                    construction_point_at(draft.points[0], center),
                    construction_point_at(draft.points[1], corner),
                    ConstructionPoint::New(second),
                    ConstructionPoint::New(opposite),
                    ConstructionPoint::New(fourth),
                ],
                corners: [1, 2, 3, 4],
                center: Some(0),
            })
        }
        V::ThreePointCenterRectangle if draft.positions.len() == 3 => {
            let center = draft.positions[0];
            let side_midpoint = draft.positions[1];
            let half_height = [side_midpoint[0] - center[0], side_midpoint[1] - center[1]];
            let height = half_height[0].hypot(half_height[1]);
            if !(height.is_finite() && height > 0.0) {
                return None;
            }
            let tangent = [-half_height[1] / height, half_height[0] / height];
            let raw = draft.positions[2];
            let mut half_width = (raw[0] - side_midpoint[0])
                .mul_add(tangent[0], (raw[1] - side_midpoint[1]) * tangent[1]);
            if draft.regularized {
                let sign = if half_width.is_sign_negative() {
                    -1.0
                } else {
                    1.0
                };
                half_width = sign * height;
            }
            if !half_width.is_finite() || half_width == 0.0 {
                return None;
            }
            let opposite_midpoint = [
                2.0 * center[0] - side_midpoint[0],
                2.0 * center[1] - side_midpoint[1],
            ];
            let first = [
                side_midpoint[0] + tangent[0] * half_width,
                side_midpoint[1] + tangent[1] * half_width,
            ];
            let second = [
                side_midpoint[0] - tangent[0] * half_width,
                side_midpoint[1] - tangent[1] * half_width,
            ];
            let third = [
                opposite_midpoint[0] - tangent[0] * half_width,
                opposite_midpoint[1] - tangent[1] * half_width,
            ];
            let fourth = [
                opposite_midpoint[0] + tangent[0] * half_width,
                opposite_midpoint[1] + tangent[1] * half_width,
            ];
            Some(ConstructionProposal::RectangleLoop {
                points: vec![
                    construction_point_at(draft.points[0], center),
                    construction_point_at(draft.points[2], first),
                    ConstructionPoint::New(second),
                    ConstructionPoint::New(third),
                    ConstructionPoint::New(fourth),
                ],
                corners: [1, 2, 3, 4],
                center: Some(0),
            })
        }
        _ => None,
    }
}

fn circumcircle(first: [f64; 2], second: [f64; 2], third: [f64; 2]) -> Option<([f64; 2], f64)> {
    if !first.into_iter().all(f64::is_finite)
        || !second.into_iter().all(f64::is_finite)
        || !third.into_iter().all(f64::is_finite)
    {
        return None;
    }
    let ab = [second[0] - first[0], second[1] - first[1]];
    let ac = [third[0] - first[0], third[1] - first[1]];
    let bc = [third[0] - second[0], third[1] - second[1]];
    let scale = ab[0]
        .hypot(ab[1])
        .max(ac[0].hypot(ac[1]))
        .max(bc[0].hypot(bc[1]));
    let cross = ab[0].mul_add(ac[1], -ab[1] * ac[0]);
    if !(scale.is_finite() && scale > 0.0 && cross.is_finite())
        || cross.abs() <= 1.0e-10 * scale * scale
    {
        return None;
    }
    let a2 = first[0].mul_add(first[0], first[1] * first[1]);
    let b2 = second[0].mul_add(second[0], second[1] * second[1]);
    let c2 = third[0].mul_add(third[0], third[1] * third[1]);
    let denominator = 2.0 * cross;
    let center = [
        (a2 * (second[1] - third[1]) + b2 * (third[1] - first[1]) + c2 * (first[1] - second[1]))
            / denominator,
        (a2 * (third[0] - second[0]) + b2 * (first[0] - third[0]) + c2 * (second[0] - first[0]))
            / denominator,
    ];
    let radius = (first[0] - center[0]).hypot(first[1] - center[1]);
    (center.into_iter().all(f64::is_finite) && radius.is_finite() && radius > 0.0)
        .then_some((center, radius))
}

#[derive(Clone, Copy, Debug)]
struct EllipseRecipeFrame {
    center: [f64; 2],
    major_axis_point: [f64; 2],
    ratio: f64,
    trim_start_stage: usize,
}

fn ellipse_recipe_frame(draft: &Draft) -> Option<EllipseRecipeFrame> {
    use GeometryToolVariant as V;

    let (center, major_axis_point, minor_sample, trim_start_stage) = match draft.variant {
        V::CenterAxesEllipse | V::CenterAxesEllipticalArc => (
            *draft.positions.first()?,
            *draft.positions.get(1)?,
            *draft.positions.get(2)?,
            3,
        ),
        V::AxisEndpointsEllipse | V::AxisEndpointsEllipticalArc => {
            let first = *draft.positions.first()?;
            let opposite = *draft.positions.get(1)?;
            (
                [
                    0.5 * (first[0] + opposite[0]),
                    0.5 * (first[1] + opposite[1]),
                ],
                first,
                *draft.positions.get(2)?,
                3,
            )
        }
        _ => return None,
    };
    let axis = [
        major_axis_point[0] - center[0],
        major_axis_point[1] - center[1],
    ];
    let major = axis[0].hypot(axis[1]);
    if !(major.is_finite() && major > 0.0) {
        return None;
    }
    let normal = [-axis[1] / major, axis[0] / major];
    let minor = ((minor_sample[0] - center[0])
        .mul_add(normal[0], (minor_sample[1] - center[1]) * normal[1]))
    .abs();
    let ratio = minor / major;
    if !(ratio.is_finite() && ratio > 0.0 && ratio <= 1.0) {
        return None;
    }
    Some(EllipseRecipeFrame {
        center,
        major_axis_point,
        ratio,
        trim_start_stage,
    })
}

fn ellipse_project_sample(frame: EllipseRecipeFrame, sample: [f64; 2]) -> Option<(f64, [f64; 2])> {
    let axis = [
        frame.major_axis_point[0] - frame.center[0],
        frame.major_axis_point[1] - frame.center[1],
    ];
    let major = axis[0].hypot(axis[1]);
    let unit = [axis[0] / major, axis[1] / major];
    let normal = [-unit[1], unit[0]];
    let delta = [sample[0] - frame.center[0], sample[1] - frame.center[1]];
    let x = delta[0].mul_add(unit[0], delta[1] * unit[1]) / major;
    let y = delta[0].mul_add(normal[0], delta[1] * normal[1]) / (major * frame.ratio);
    if !x.is_finite() || !y.is_finite() || (x == 0.0 && y == 0.0) {
        return None;
    }
    let parameter = y.atan2(x);
    let projected = [
        frame.center[0]
            + major * parameter.cos() * unit[0]
            + major * frame.ratio * parameter.sin() * normal[0],
        frame.center[1]
            + major * parameter.cos() * unit[1]
            + major * frame.ratio * parameter.sin() * normal[1],
    ];
    (parameter.is_finite() && projected.into_iter().all(f64::is_finite))
        .then_some((parameter, projected))
}

fn rational_conic_weighted_middle(
    start: [f64; 2],
    middle: [f64; 2],
    weight: f64,
) -> Option<[f64; 2]> {
    if !weight.is_finite()
        || !start.into_iter().all(f64::is_finite)
        || !middle.into_iter().all(f64::is_finite)
    {
        return None;
    }
    // A nonzero construction click has the same ordinary P1 meaning as the
    // later selected-curve control. At the valid zero-weight mode no finite P1
    // exists, so the click is the tip of the explicitly projective Qh vector
    // anchored at Start, matching its later selected-curve presentation.
    let weighted = if weight == 0.0 {
        [middle[0] - start[0], middle[1] - start[1]]
    } else {
        [weight * middle[0], weight * middle[1]]
    };
    weighted.into_iter().all(f64::is_finite).then_some(weighted)
}

fn tangent_arc_proposal(draft: &Draft) -> Option<ConstructionProposal> {
    let source = draft.tangent_source?;
    let end = *draft.positions.get(1)?;
    let (center, sweep) =
        tangent_arc_center_and_sweep(source.position, source.outgoing_tangent, end)?;
    let radial = [
        source.position[0] - center[0],
        source.position[1] - center[1],
    ];
    let start_angle = radial[1].atan2(radial[0]);
    let end_angle = (end[1] - center[1]).atan2(end[0] - center[0]);
    if !elliptical_arc_sweep_is_nonzero(start_angle, end_angle, sweep) {
        return None;
    }
    Some(ConstructionProposal::CircularArc {
        center: ConstructionPoint::New(center),
        start: source.position,
        end,
        sweep,
    })
}

fn tangent_arc_center_and_sweep(
    source: [f64; 2],
    outgoing: [f64; 2],
    end: [f64; 2],
) -> Option<([f64; 2], DocumentArcSweep)> {
    if !source.into_iter().all(f64::is_finite)
        || !outgoing.into_iter().all(f64::is_finite)
        || !end.into_iter().all(f64::is_finite)
    {
        return None;
    }
    let chord = [end[0] - source[0], end[1] - source[1]];
    let chord_length = chord[0].hypot(chord[1]);
    if !(chord_length.is_finite() && chord_length > 0.0) {
        return None;
    }
    let normal = [-outgoing[1], outgoing[0]];
    let normal_chord = chord[0].mul_add(normal[0], chord[1] * normal[1]);
    if !normal_chord.is_finite() || normal_chord.abs() <= 1.0e-8 * chord_length {
        return None;
    }
    // Keep one chord factor scaled by the like-sized normal projection.
    // Squaring first would underflow/overflow even when the resulting circle
    // center is finite and representable.
    let offset = 0.5 * chord_length * (chord_length / normal_chord);
    let center = [
        normal[0].mul_add(offset, source[0]),
        normal[1].mul_add(offset, source[1]),
    ];
    let radial = [source[0] - center[0], source[1] - center[1]];
    let radius = radial[0].hypot(radial[1]);
    if !(center.into_iter().all(f64::is_finite) && radius.is_finite() && radius > 0.0) {
        return None;
    }
    let sweep = if offset > 0.0 {
        DocumentArcSweep::CounterClockwise
    } else {
        DocumentArcSweep::Clockwise
    };
    Some((center, sweep))
}

#[allow(
    clippy::too_many_lines,
    reason = "the legacy EditorTool lowering remains explicit for source-compatible hosts"
)]
fn legacy_draft_proposal(draft: &Draft) -> Option<ConstructionProposal> {
    match draft.tool {
        EditorTool::Point => draft
            .points
            .first()
            .copied()
            .map(|point| ConstructionProposal::Point { point }),
        EditorTool::Line if draft.points.len() == 2 => {
            let delta = [
                draft.positions[1][0] - draft.positions[0][0],
                draft.positions[1][1] - draft.positions[0][1],
            ];
            (delta[0].hypot(delta[1]) > 0.0).then(|| ConstructionProposal::Line {
                start: draft.points[0],
                end: draft.points[1],
            })
        }
        EditorTool::Polyline => polyline_proposal(draft),
        EditorTool::Rectangle if draft.positions.len() == 2 => {
            let first = draft.positions[0];
            let second = draft.positions[1];
            ((second[0] - first[0]).abs() > 0.0 && (second[1] - first[1]).abs() > 0.0)
                .then_some(ConstructionProposal::Rectangle { first, second })
        }
        EditorTool::Circle if draft.points.len() == 2 => {
            let radius = (draft.positions[1][0] - draft.positions[0][0])
                .hypot(draft.positions[1][1] - draft.positions[0][1]);
            (radius.is_finite() && radius > 0.0).then(|| ConstructionProposal::Circle {
                center: draft.points[0],
                radius,
            })
        }
        EditorTool::CounterClockwiseArc if draft.points.len() == 3 => {
            let center = draft.positions[0];
            let start = draft.positions[1];
            let end = draft.positions[2];
            let radius = (start[0] - center[0]).hypot(start[1] - center[1]);
            let end_radius = (end[0] - center[0]).hypot(end[1] - center[1]);
            if !(radius.is_finite() && radius > 0.0 && end_radius.is_finite() && end_radius > 0.0) {
                return None;
            }
            let scale = radius / end_radius;
            let end = [
                center[0] + (end[0] - center[0]) * scale,
                center[1] + (end[1] - center[1]) * scale,
            ];
            (end.into_iter().all(f64::is_finite) && nonzero_segment(&[start, end])).then_some(
                ConstructionProposal::CounterClockwiseArc {
                    center: draft.points[0],
                    start,
                    end,
                },
            )
        }
        EditorTool::QuadraticBezier if draft.points.len() == 3 => {
            Some(ConstructionProposal::QuadraticBezier {
                controls: [draft.points[0], draft.points[1], draft.points[2]],
            })
        }
        EditorTool::CubicBezier if draft.points.len() == 4 => {
            Some(ConstructionProposal::CubicBezier {
                controls: [
                    draft.points[0],
                    draft.points[1],
                    draft.points[2],
                    draft.points[3],
                ],
            })
        }
        EditorTool::Ellipse
            if draft.points.len() == 2 && nonzero_segment(&draft.positions[..2]) =>
        {
            Some(ConstructionProposal::Ellipse {
                center: draft.points[0],
                major_axis_point: draft.points[1],
                minor_axis_ratio: draft.conic_options.minor_axis_ratio,
            })
        }
        EditorTool::EllipticalArc
            if draft.points.len() == 4 && nonzero_segment(&draft.positions[..2]) =>
        {
            let start = elliptical_arc_click_projection(draft, 2)?;
            let end = elliptical_arc_click_projection(draft, 3)?;
            if !elliptical_arc_sweep_is_nonzero(
                start.parameter,
                end.parameter,
                draft.conic_options.arc_sweep,
            ) {
                return None;
            }
            Some(ConstructionProposal::EllipticalArc {
                center: draft.points[0],
                major_axis_point: draft.points[1],
                minor_axis_ratio: draft.conic_options.minor_axis_ratio,
                start_angle: start.parameter,
                end_angle: end.parameter,
                sweep: draft.conic_options.arc_sweep,
            })
        }
        EditorTool::RationalQuadraticConic
            if draft.points.len() == 3
                && nonzero_segment(&[draft.positions[0], draft.positions[2]]) =>
        {
            let weighted_middle = rational_conic_weighted_middle(
                draft.positions[0],
                draft.positions[1],
                draft.conic_options.middle_weight,
            )?;
            Some(ConstructionProposal::RationalQuadraticConic {
                start: draft.points[0],
                weighted_middle,
                middle_weight: draft.conic_options.middle_weight,
                end: draft.points[2],
            })
        }
        EditorTool::Parabola
            if draft.points.len() == 2 && nonzero_segment(&draft.positions[..2]) =>
        {
            Some(ConstructionProposal::Parabola {
                vertex: draft.points[0],
                focus: draft.points[1],
                trim_start: draft.conic_options.trim_start,
                trim_end: draft.conic_options.trim_end,
            })
        }
        EditorTool::Hyperbola
            if draft.points.len() == 2 && nonzero_segment(&draft.positions[..2]) =>
        {
            Some(ConstructionProposal::Hyperbola {
                center: draft.points[0],
                transverse_axis_point: draft.points[1],
                semi_conjugate: draft.conic_options.semi_conjugate,
                branch: draft.conic_options.hyperbola_branch,
                trim_start: draft.conic_options.trim_start,
                trim_end: draft.conic_options.trim_end,
            })
        }
        EditorTool::Nurbs => nurbs_proposal(draft),
        _ => None,
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive table keeps every tool-to-proposal completion rule auditable"
)]
fn draft_proposal(draft: &Draft) -> Option<ConstructionProposal> {
    use GeometryToolVariant as V;

    if !draft.exact_variant {
        return legacy_draft_proposal(draft);
    }

    match draft.variant {
        V::SketchPoint => draft
            .points
            .first()
            .copied()
            .map(|point| ConstructionProposal::Point { point }),
        V::Segment if draft.points.len() == 2 => {
            let delta = [
                draft.positions[1][0] - draft.positions[0][0],
                draft.positions[1][1] - draft.positions[0][1],
            ];
            (delta[0].hypot(delta[1]) > 0.0).then(|| ConstructionProposal::Line {
                start: draft.points[0],
                end: draft.points[1],
            })
        }
        V::Polyline => polyline_proposal(draft),
        V::MidpointLine if draft.points.len() == 2 => {
            let center = draft.positions[0];
            let endpoint = draft.positions[1];
            nonzero_segment(&[center, endpoint]).then(|| ConstructionProposal::MidpointLine {
                center: draft.points[0],
                endpoint: draft.points[1],
                opposite: ConstructionPoint::New([
                    2.0 * center[0] - endpoint[0],
                    2.0 * center[1] - endpoint[1],
                ]),
            })
        }
        V::TwoPointAlignedRectangle
        | V::ThreePointCornerRectangle
        | V::CenterRectangle
        | V::ThreePointCenterRectangle => rectangle_loop_proposal(draft),
        V::CenterRadiusCircle if draft.points.len() == 2 => {
            let radius = (draft.positions[1][0] - draft.positions[0][0])
                .hypot(draft.positions[1][1] - draft.positions[0][1]);
            (radius.is_finite() && radius > 0.0).then(|| ConstructionProposal::Circle {
                center: draft.points[0],
                radius,
            })
        }
        V::TwoPointDiameterCircle if draft.positions.len() == 2 => {
            let first = draft.positions[0];
            let second = draft.positions[1];
            let radius = 0.5 * (second[0] - first[0]).hypot(second[1] - first[1]);
            (radius.is_finite() && radius > 0.0).then(|| ConstructionProposal::Circle {
                center: ConstructionPoint::New([
                    0.5 * (first[0] + second[0]),
                    0.5 * (first[1] + second[1]),
                ]),
                radius,
            })
        }
        V::ThreePointCircle if draft.positions.len() == 3 => {
            let (center, radius) =
                circumcircle(draft.positions[0], draft.positions[1], draft.positions[2])?;
            Some(ConstructionProposal::Circle {
                center: ConstructionPoint::New(center),
                radius,
            })
        }
        V::CenterArc if draft.positions.len() == 3 => {
            let center = draft.positions[0];
            let start = draft.positions[1];
            let end = draft.positions[2];
            let radius = (start[0] - center[0]).hypot(start[1] - center[1]);
            let end_radius = (end[0] - center[0]).hypot(end[1] - center[1]);
            if !(radius.is_finite() && radius > 0.0 && end_radius.is_finite() && end_radius > 0.0) {
                return None;
            }
            let scale = radius / end_radius;
            let end = [
                center[0] + (end[0] - center[0]) * scale,
                center[1] + (end[1] - center[1]) * scale,
            ];
            (end.into_iter().all(f64::is_finite) && nonzero_segment(&[start, end])).then_some(
                ConstructionProposal::CircularArc {
                    center: draft.points[0],
                    start,
                    end,
                    sweep: draft.conic_options.arc_sweep,
                },
            )
        }
        V::ThreePointArc if draft.positions.len() == 3 => {
            let start = draft.positions[0];
            let end = draft.positions[1];
            let through = draft.positions[2];
            let (center, _) = circumcircle(start, end, through)?;
            let start_angle = (start[1] - center[1]).atan2(start[0] - center[0]);
            let end_angle = (end[1] - center[1]).atan2(end[0] - center[0]);
            let through_angle = (through[1] - center[1]).atan2(through[0] - center[0]);
            let ccw_end = (end_angle - start_angle).rem_euclid(std::f64::consts::TAU);
            let ccw_through = (through_angle - start_angle).rem_euclid(std::f64::consts::TAU);
            let sweep = if ccw_through <= ccw_end {
                DocumentArcSweep::CounterClockwise
            } else {
                DocumentArcSweep::Clockwise
            };
            Some(ConstructionProposal::CircularArc {
                center: ConstructionPoint::New(center),
                start,
                end,
                sweep,
            })
        }
        V::TangentArc if draft.positions.len() == 2 => tangent_arc_proposal(draft),
        V::QuadraticBezier if draft.points.len() == 3 => {
            Some(ConstructionProposal::QuadraticBezier {
                controls: [draft.points[0], draft.points[1], draft.points[2]],
            })
        }
        V::CubicBezier if draft.points.len() == 4 => Some(ConstructionProposal::CubicBezier {
            controls: [
                draft.points[0],
                draft.points[1],
                draft.points[2],
                draft.points[3],
            ],
        }),
        V::CenterAxesEllipse if draft.positions.len() == 3 => {
            let frame = ellipse_recipe_frame(draft)?;
            Some(ConstructionProposal::Ellipse {
                center: draft.points[0],
                major_axis_point: draft.points[1],
                minor_axis_ratio: frame.ratio,
            })
        }
        V::AxisEndpointsEllipse if draft.positions.len() == 3 => {
            let frame = ellipse_recipe_frame(draft)?;
            Some(ConstructionProposal::AxisEndpointEllipse {
                major_axis_point: draft.points[0],
                center: ConstructionPoint::New(frame.center),
                minor_axis_ratio: frame.ratio,
            })
        }
        V::CenterAxesEllipticalArc if draft.positions.len() == 5 => {
            let frame = ellipse_recipe_frame(draft)?;
            let (start, _) = ellipse_project_sample(frame, draft.positions[3])?;
            let (end, _) = ellipse_project_sample(frame, draft.positions[4])?;
            if !elliptical_arc_sweep_is_nonzero(start, end, draft.conic_options.arc_sweep) {
                return None;
            }
            Some(ConstructionProposal::EllipticalArc {
                center: draft.points[0],
                major_axis_point: draft.points[1],
                minor_axis_ratio: frame.ratio,
                start_angle: start,
                end_angle: end,
                sweep: draft.conic_options.arc_sweep,
            })
        }
        V::AxisEndpointsEllipticalArc if draft.positions.len() == 5 => {
            let frame = ellipse_recipe_frame(draft)?;
            let (start, _) = ellipse_project_sample(frame, draft.positions[3])?;
            let (end, _) = ellipse_project_sample(frame, draft.positions[4])?;
            if !elliptical_arc_sweep_is_nonzero(start, end, draft.conic_options.arc_sweep) {
                return None;
            }
            Some(ConstructionProposal::AxisEndpointEllipticalArc {
                major_axis_point: draft.points[0],
                center: ConstructionPoint::New(frame.center),
                minor_axis_ratio: frame.ratio,
                start_angle: start,
                end_angle: end,
                sweep: draft.conic_options.arc_sweep,
            })
        }
        V::RationalQuadraticConic
            if draft.points.len() == 3
                && nonzero_segment(&[draft.positions[0], draft.positions[2]]) =>
        {
            let weighted_middle = rational_conic_weighted_middle(
                draft.positions[0],
                draft.positions[1],
                draft.conic_options.middle_weight,
            )?;
            Some(ConstructionProposal::RationalQuadraticConic {
                start: draft.points[0],
                weighted_middle,
                middle_weight: draft.conic_options.middle_weight,
                end: draft.points[2],
            })
        }
        V::Parabola if draft.points.len() == 2 && nonzero_segment(&draft.positions[..2]) => {
            Some(ConstructionProposal::Parabola {
                vertex: draft.points[0],
                focus: draft.points[1],
                trim_start: draft.conic_options.trim_start,
                trim_end: draft.conic_options.trim_end,
            })
        }
        V::Hyperbola if draft.points.len() == 2 && nonzero_segment(&draft.positions[..2]) => {
            Some(ConstructionProposal::Hyperbola {
                center: draft.points[0],
                transverse_axis_point: draft.points[1],
                semi_conjugate: draft.conic_options.semi_conjugate,
                branch: draft.conic_options.hyperbola_branch,
                trim_start: draft.conic_options.trim_start,
                trim_end: draft.conic_options.trim_end,
            })
        }
        V::OpenControlNurbs | V::PeriodicControlNurbs => nurbs_proposal(draft),
        _ => None,
    }
}

fn draft_preview(draft: &Draft) -> Option<ConstructionPreview> {
    use GeometryToolVariant as V;

    if !draft.exact_variant {
        return legacy_draft_preview(draft);
    }

    if draft_proposal(draft).is_some() {
        return complete_preview(draft);
    }
    match draft.variant {
        V::CenterRadiusCircle | V::MidpointLine if draft.positions.len() == 1 => {
            Some(ConstructionPreview::Anchor {
                position: draft.positions[0],
            })
        }
        V::TwoPointDiameterCircle | V::ThreePointCircle | V::ThreePointArc => {
            Some(ConstructionPreview::GuidePolyline {
                points: draft.positions.clone(),
                closed: false,
            })
        }
        V::CenterArc => match draft.positions.as_slice() {
            [center] => Some(ConstructionPreview::Anchor { position: *center }),
            [center, start] => Some(ConstructionPreview::ArcRadiusGuide {
                center: *center,
                start: *start,
            }),
            _ => None,
        },
        V::TangentArc
        | V::TwoPointAlignedRectangle
        | V::ThreePointCornerRectangle
        | V::CenterRectangle
        | V::ThreePointCenterRectangle
        | V::CenterAxesEllipse
        | V::AxisEndpointsEllipse => Some(ConstructionPreview::GuidePolyline {
            points: draft.positions.clone(),
            closed: false,
        }),
        V::CenterAxesEllipticalArc | V::AxisEndpointsEllipticalArc => {
            if draft.positions.len() >= 3 {
                elliptical_arc_support_preview(draft)
            } else {
                Some(ConstructionPreview::GuidePolyline {
                    points: draft.positions.clone(),
                    closed: false,
                })
            }
        }
        _ if advanced_kind(draft.tool).is_some() => Some(ConstructionPreview::ControlPolygon {
            kind: advanced_kind(draft.tool).expect("guarded advanced tool"),
            points: draft.positions.clone(),
        }),
        _ => draft
            .positions
            .first()
            .copied()
            .map(|position| ConstructionPreview::Anchor { position }),
    }
}

fn legacy_draft_preview(draft: &Draft) -> Option<ConstructionPreview> {
    match draft.tool {
        EditorTool::Circle if draft.points.len() == 1 => Some(ConstructionPreview::Anchor {
            position: draft.positions[0],
        }),
        EditorTool::CounterClockwiseArc => match draft.points.as_slice() {
            [_] => Some(ConstructionPreview::Anchor {
                position: draft.positions[0],
            }),
            [_, _] => Some(ConstructionPreview::ArcRadiusGuide {
                center: draft.positions[0],
                start: draft.positions[1],
            }),
            _ => complete_preview(draft),
        },
        EditorTool::EllipticalArc if draft_proposal(draft).is_none() => {
            match draft.positions.as_slice() {
                [center] => Some(ConstructionPreview::Anchor { position: *center }),
                [_, _] | [_, _, _] => legacy_elliptical_arc_support_preview(draft),
                _ => None,
            }
        }
        tool if advanced_kind(tool).is_some() && draft_proposal(draft).is_none() => {
            Some(ConstructionPreview::ControlPolygon {
                kind: advanced_kind(tool).expect("guarded advanced tool"),
                points: draft.positions.clone(),
            })
        }
        _ => complete_preview(draft),
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive proposal-to-preview lowering prevents a second recipe authority"
)]
fn complete_preview(draft: &Draft) -> Option<ConstructionPreview> {
    let proposal = draft_proposal(draft)?;
    let geometry = match &proposal {
        ConstructionProposal::Point { point } => ConstructionPreviewGeometry::Point {
            position: point.position(),
        },
        ConstructionProposal::Line { .. }
        | ConstructionProposal::Polyline { .. }
        | ConstructionProposal::PolylinePath { .. }
        | ConstructionProposal::MidpointLine { .. } => ConstructionPreviewGeometry::Polyline {
            points: match &proposal {
                ConstructionProposal::MidpointLine {
                    endpoint, opposite, ..
                } => vec![opposite.position(), endpoint.position()],
                ConstructionProposal::PolylinePath {
                    points,
                    closed: true,
                } => points
                    .iter()
                    .chain(points.first())
                    .map(|point| point.position())
                    .collect(),
                _ => draft.positions.clone(),
            },
        },
        ConstructionProposal::Rectangle { first, second } => {
            ConstructionPreviewGeometry::Rectangle {
                first: *first,
                second: *second,
            }
        }
        ConstructionProposal::RectangleLoop {
            points, corners, ..
        } => ConstructionPreviewGeometry::Polyline {
            points: corners
                .iter()
                .chain(corners.first())
                .map(|index| points[*index].position())
                .collect(),
        },
        ConstructionProposal::Circle { radius, .. } => ConstructionPreviewGeometry::Circle {
            center: match &proposal {
                ConstructionProposal::Circle { center, .. } => center.position(),
                _ => unreachable!("guarded circle proposal"),
            },
            radius: *radius,
        },
        ConstructionProposal::CounterClockwiseArc { start, end, .. } => {
            let center = draft.positions[0];
            let start_angle = (start[1] - center[1]).atan2(start[0] - center[0]);
            let end_angle = (end[1] - center[1]).atan2(end[0] - center[0]);
            let sweep_radians = (end_angle - start_angle).rem_euclid(std::f64::consts::TAU);
            if !sweep_radians.is_finite() || sweep_radians <= 0.0 {
                return None;
            }
            ConstructionPreviewGeometry::CounterClockwiseArc {
                center,
                start: *start,
                end: *end,
                radius: (start[0] - center[0]).hypot(start[1] - center[1]),
                sweep_radians,
                large_arc: sweep_radians > std::f64::consts::PI,
            }
        }
        ConstructionProposal::CircularArc {
            center,
            start,
            end,
            sweep,
        } => {
            let center = center.position();
            let start_angle = (start[1] - center[1]).atan2(start[0] - center[0]);
            let end_angle = (end[1] - center[1]).atan2(end[0] - center[0]);
            let sweep_radians = match sweep {
                DocumentArcSweep::CounterClockwise => {
                    (end_angle - start_angle).rem_euclid(std::f64::consts::TAU)
                }
                DocumentArcSweep::Clockwise => {
                    (start_angle - end_angle).rem_euclid(std::f64::consts::TAU)
                }
            };
            if !sweep_radians.is_finite() || sweep_radians <= 0.0 {
                return None;
            }
            ConstructionPreviewGeometry::CircularArc {
                center,
                start: *start,
                end: *end,
                radius: (start[0] - center[0]).hypot(start[1] - center[1]),
                sweep_radians,
                large_arc: sweep_radians > std::f64::consts::PI,
                sweep: *sweep,
            }
        }
        ConstructionProposal::QuadraticBezier { .. }
        | ConstructionProposal::CubicBezier { .. }
        | ConstructionProposal::Ellipse { .. }
        | ConstructionProposal::AxisEndpointEllipse { .. }
        | ConstructionProposal::EllipticalArc { .. }
        | ConstructionProposal::AxisEndpointEllipticalArc { .. }
        | ConstructionProposal::RationalQuadraticConic { .. }
        | ConstructionProposal::Parabola { .. }
        | ConstructionProposal::Hyperbola { .. }
        | ConstructionProposal::Nurbs { .. } => {
            let projected_positions = if draft.tool == EditorTool::EllipticalArc {
                if draft.exact_variant {
                    Some(elliptical_arc_preview_positions(draft)?)
                } else {
                    Some(legacy_elliptical_arc_preview_positions(draft)?)
                }
            } else {
                None
            };
            let control_points = projected_positions
                .as_deref()
                .unwrap_or(draft.positions.as_slice());
            advanced_curve_preview(&proposal, control_points, draft.tool)?
        }
    };
    Some(ConstructionPreview::Complete { proposal, geometry })
}

fn advanced_kind(tool: EditorTool) -> Option<AdvancedConstructionKind> {
    Some(match tool {
        EditorTool::QuadraticBezier => AdvancedConstructionKind::QuadraticBezier,
        EditorTool::CubicBezier => AdvancedConstructionKind::CubicBezier,
        EditorTool::Ellipse => AdvancedConstructionKind::Ellipse,
        EditorTool::EllipticalArc => AdvancedConstructionKind::EllipticalArc,
        EditorTool::RationalQuadraticConic => AdvancedConstructionKind::RationalQuadraticConic,
        EditorTool::Parabola => AdvancedConstructionKind::Parabola,
        EditorTool::Hyperbola => AdvancedConstructionKind::Hyperbola,
        EditorTool::Nurbs => AdvancedConstructionKind::Nurbs,
        _ => return None,
    })
}

const fn is_conic_tool(tool: EditorTool) -> bool {
    matches!(
        tool,
        EditorTool::Ellipse
            | EditorTool::EllipticalArc
            | EditorTool::RationalQuadraticConic
            | EditorTool::Parabola
            | EditorTool::Hyperbola
    )
}

fn advanced_curve_preview(
    proposal: &ConstructionProposal,
    control_points: &[[f64; 2]],
    tool: EditorTool,
) -> Option<ConstructionPreviewGeometry> {
    let local = localize_proposal(proposal, control_points)?;
    let mut document = SketchDocument::new(1.0).ok()?;
    let result = local.apply(&mut document).ok()?;
    let curve = *result.curves.first()?;
    let mut curve_points = Vec::new();
    for span in document.curve_spans(curve).ok()? {
        for interval in document.visible_intervals(span).ok()? {
            for step in 0..=ADVANCED_CURVE_PREVIEW_SUBDIVISIONS {
                let ratio = f64::from(step) / f64::from(ADVANCED_CURVE_PREVIEW_SUBDIVISIONS);
                let parameter = (interval.end - interval.start).mul_add(ratio, interval.start);
                let jet = document.evaluate_curve_jet(span, parameter).ok()?;
                curve_points.push([jet.position.x, jet.position.y]);
            }
        }
    }
    Some(ConstructionPreviewGeometry::AdvancedCurve {
        kind: advanced_kind(tool)?,
        control_points: control_points.to_vec(),
        curve_points,
    })
}

fn localize_proposal(
    proposal: &ConstructionProposal,
    positions: &[[f64; 2]],
) -> Option<ConstructionProposal> {
    let point = |index: usize| positions.get(index).copied().map(ConstructionPoint::New);
    Some(match proposal {
        ConstructionProposal::QuadraticBezier { .. } => ConstructionProposal::QuadraticBezier {
            controls: [point(0)?, point(1)?, point(2)?],
        },
        ConstructionProposal::CubicBezier { .. } => ConstructionProposal::CubicBezier {
            controls: [point(0)?, point(1)?, point(2)?, point(3)?],
        },
        ConstructionProposal::Ellipse {
            minor_axis_ratio, ..
        } => ConstructionProposal::Ellipse {
            center: point(0)?,
            major_axis_point: point(1)?,
            minor_axis_ratio: *minor_axis_ratio,
        },
        ConstructionProposal::AxisEndpointEllipse {
            major_axis_point,
            center,
            minor_axis_ratio,
        } => ConstructionProposal::AxisEndpointEllipse {
            major_axis_point: ConstructionPoint::New(major_axis_point.position()),
            center: ConstructionPoint::New(center.position()),
            minor_axis_ratio: *minor_axis_ratio,
        },
        ConstructionProposal::EllipticalArc {
            minor_axis_ratio,
            start_angle,
            end_angle,
            sweep,
            ..
        } => ConstructionProposal::EllipticalArc {
            center: point(0)?,
            major_axis_point: point(1)?,
            minor_axis_ratio: *minor_axis_ratio,
            start_angle: *start_angle,
            end_angle: *end_angle,
            sweep: *sweep,
        },
        ConstructionProposal::AxisEndpointEllipticalArc {
            major_axis_point,
            center,
            minor_axis_ratio,
            start_angle,
            end_angle,
            sweep,
        } => ConstructionProposal::AxisEndpointEllipticalArc {
            major_axis_point: ConstructionPoint::New(major_axis_point.position()),
            center: ConstructionPoint::New(center.position()),
            minor_axis_ratio: *minor_axis_ratio,
            start_angle: *start_angle,
            end_angle: *end_angle,
            sweep: *sweep,
        },
        ConstructionProposal::RationalQuadraticConic { middle_weight, .. } => {
            ConstructionProposal::RationalQuadraticConic {
                start: point(0)?,
                weighted_middle: rational_conic_weighted_middle(
                    *positions.first()?,
                    *positions.get(1)?,
                    *middle_weight,
                )?,
                middle_weight: *middle_weight,
                end: point(2)?,
            }
        }
        ConstructionProposal::Parabola {
            trim_start,
            trim_end,
            ..
        } => ConstructionProposal::Parabola {
            vertex: point(0)?,
            focus: point(1)?,
            trim_start: *trim_start,
            trim_end: *trim_end,
        },
        ConstructionProposal::Hyperbola {
            semi_conjugate,
            branch,
            trim_start,
            trim_end,
            ..
        } => ConstructionProposal::Hyperbola {
            center: point(0)?,
            transverse_axis_point: point(1)?,
            semi_conjugate: *semi_conjugate,
            branch: *branch,
            trim_start: *trim_start,
            trim_end: *trim_end,
        },
        ConstructionProposal::Nurbs { options, .. } => ConstructionProposal::Nurbs {
            controls: positions
                .iter()
                .copied()
                .map(ConstructionPoint::New)
                .collect(),
            options: options.clone(),
        },
        _ => return None,
    })
}

fn validate_conic_options(options: ConicConstructionOptions) -> Result<(), EditorError> {
    let finite = [
        options.minor_axis_ratio,
        options.arc_start,
        options.arc_end,
        options.middle_weight,
        options.trim_start,
        options.trim_end,
        options.semi_conjugate,
    ]
    .into_iter()
    .all(f64::is_finite);
    if !finite {
        return Err(EditorError::InvalidConstructionOptions(
            "conic values must be finite",
        ));
    }
    if options.minor_axis_ratio <= 0.0 || options.minor_axis_ratio > 1.0 {
        return Err(EditorError::InvalidConstructionOptions(
            "minor-axis ratio must be in (0, 1]",
        ));
    }
    if options.middle_weight < MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT {
        return Err(EditorError::InvalidConstructionOptions(
            "rational middle weight is outside its supported domain",
        ));
    }
    if options.semi_conjugate <= 0.0 {
        return Err(EditorError::InvalidConstructionOptions(
            "hyperbola semi-conjugate length must be positive",
        ));
    }
    Ok(())
}

fn validate_nurbs_options(options: &NurbsConstructionOptions) -> Result<(), EditorError> {
    if options.degree == 0 {
        return Err(EditorError::InvalidConstructionOptions(
            "NURBS degree must be positive",
        ));
    }
    if options
        .weights
        .iter()
        .any(|weight| !weight.is_finite() || *weight <= 0.0)
    {
        return Err(EditorError::InvalidConstructionOptions(
            "NURBS weights must be finite and positive",
        ));
    }
    if !options.weights.is_empty() && options.gauge_index >= options.weights.len() {
        return Err(EditorError::InvalidConstructionOptions(
            "NURBS gauge index is outside the weight list",
        ));
    }
    if !options.weights.is_empty()
        && options.weights[options.gauge_index].to_bits() != 1.0_f64.to_bits()
    {
        return Err(EditorError::InvalidConstructionOptions(
            "the selected NURBS gauge weight must be exactly one",
        ));
    }
    Ok(())
}

fn validate_nurbs_for_controls(
    options: &NurbsConstructionOptions,
    control_count: usize,
) -> Result<(), geosolve_sketch::DocumentError> {
    let degree = usize::try_from(options.degree).map_err(|_| {
        construction_document_error("NURBS degree", "degree does not fit this platform")
    })?;
    if degree == 0 || control_count <= degree {
        return Err(construction_document_error(
            "NURBS controls",
            "control count must be greater than the positive degree",
        ));
    }
    if !options.weights.is_empty() && options.weights.len() != control_count {
        return Err(construction_document_error(
            "NURBS weights",
            "provide no weights for unit defaults or exactly one per control",
        ));
    }
    if options.gauge_index >= control_count {
        return Err(construction_document_error(
            "NURBS gauge",
            "gauge index is outside the control/weight list",
        ));
    }
    if options
        .weights
        .iter()
        .any(|weight| !weight.is_finite() || *weight <= 0.0)
    {
        return Err(construction_document_error(
            "NURBS weights",
            "weights must be finite and positive",
        ));
    }
    if !options.weights.is_empty()
        && options.weights[options.gauge_index].to_bits() != 1.0_f64.to_bits()
    {
        return Err(construction_document_error(
            "NURBS gauge",
            "the selected gauge weight must be exactly one",
        ));
    }
    Ok(())
}

fn nurbs_topology(
    form: DocumentBSplineForm,
    degree: u32,
    control_count: usize,
) -> Result<(Vec<f64>, Vec<u32>, u32), geosolve_sketch::DocumentError> {
    let degree = usize::try_from(degree).map_err(|_| {
        construction_document_error("NURBS degree", "degree does not fit this platform")
    })?;
    let span_count = match form {
        DocumentBSplineForm::Clamped => control_count.checked_sub(degree).ok_or_else(|| {
            construction_document_error("NURBS topology", "degree exceeds control count")
        })?,
        DocumentBSplineForm::Periodic => control_count,
    };
    let span_count_u32 = u32::try_from(span_count).map_err(|_| {
        construction_document_error("NURBS topology", "span count exceeds persistent limits")
    })?;
    let span_ids = (1..=span_count_u32).collect::<Vec<_>>();
    let next_span_id = span_count_u32.checked_add(1).ok_or_else(|| {
        construction_document_error("NURBS topology", "span identity high-water overflow")
    })?;
    let knots = match form {
        DocumentBSplineForm::Clamped => {
            let mut knots = vec![0.0; degree + 1];
            knots.extend((1..span_count_u32).map(f64::from));
            knots.extend(std::iter::repeat_n(f64::from(span_count_u32), degree + 1));
            knots
        }
        DocumentBSplineForm::Periodic => (0..=span_count_u32).map(f64::from).collect(),
    };
    Ok((knots, span_ids, next_span_id))
}

fn construction_document_error(
    field: &'static str,
    message: &'static str,
) -> geosolve_sketch::DocumentError {
    geosolve_sketch::DocumentError::InvalidField {
        field,
        message: message.into(),
    }
}

fn is_linear_span(document: &SketchDocument, span: CurveSpan) -> bool {
    document
        .curve(span.curve)
        .is_some_and(|curve| match &curve.definition {
            CurveDefinition::Line { .. } => span.segment == 0,
            CurveDefinition::Polyline { points, closed, .. } => {
                let segment_count = points.len().saturating_sub(1) + usize::from(*closed);
                usize::try_from(span.segment).is_ok_and(|segment| segment < segment_count)
            }
            _ => false,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use geosolve_sketch::{DocumentConstraintDefinition, SketchDocument};
    use std::ops::{Deref, DerefMut};

    /// Historical editor fixtures predate intrinsic reference geometry. Keep
    /// their native drafting assertions isolated; focused M74 integration
    /// tests exercise the production-default datum policy end to end.
    struct ConstraintEditor(super::ConstraintEditor);

    impl ConstraintEditor {
        fn new(
            pick_tolerance: PickTolerance,
            drag_threshold_pixels: f64,
        ) -> Result<Self, EditorError> {
            super::ConstraintEditor::new(pick_tolerance, drag_threshold_pixels).map(Self::from)
        }
    }

    impl Default for ConstraintEditor {
        fn default() -> Self {
            Self::from(super::ConstraintEditor::default())
        }
    }

    impl From<super::ConstraintEditor> for ConstraintEditor {
        fn from(mut editor: super::ConstraintEditor) -> Self {
            let _ = editor.set_geometry_visibility(GeometryVisibility {
                reference_geometry: false,
                ..GeometryVisibility::default()
            });
            Self(editor)
        }
    }

    impl Deref for ConstraintEditor {
        type Target = super::ConstraintEditor;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl DerefMut for ConstraintEditor {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    fn native_geometry_policy() -> GeometryInteractionPolicy {
        GeometryInteractionPolicy {
            visibility: GeometryVisibility {
                reference_geometry: false,
                ..GeometryVisibility::default()
            },
            ..GeometryInteractionPolicy::default()
        }
    }

    #[test]
    fn tangent_arc_center_is_finite_at_representable_extreme_scales() {
        for scale in [1.0e-200, 1.0e200] {
            let (center, sweep) =
                tangent_arc_center_and_sweep([0.0, 0.0], [1.0, 0.0], [scale, scale])
                    .expect("finite scaled tangent circle");
            assert_eq!(center[0].to_bits(), 0.0f64.to_bits());
            assert!((center[1] / scale - 1.0).abs() <= 1.0e-12);
            assert_eq!(sweep, DocumentArcSweep::CounterClockwise);
        }
    }

    fn line_document() -> (SketchDocument, [CurveSpan; 2], [DesignPointId; 4]) {
        let mut document = SketchDocument::new(10.0).expect("document");
        let p0 = document.add_point("a", [-4.0, 1.0]).expect("point");
        let p1 = document.add_point("b", [4.0, 1.0]).expect("point");
        let p2 = document.add_point("c", [-4.0, -1.0]).expect("point");
        let p3 = document.add_point("d", [4.0, -1.0]).expect("point");
        let first = document
            .add_curve(
                "first",
                CurveDefinition::Line {
                    start: p0,
                    end: p1,
                    branch_direction: [1.0, 0.0],
                },
            )
            .expect("curve");
        let second = document
            .add_curve(
                "second",
                CurveDefinition::Line {
                    start: p2,
                    end: p3,
                    branch_direction: [1.0, 0.0],
                },
            )
            .expect("curve");
        (
            document,
            [
                CurveSpan {
                    curve: first,
                    segment: 0,
                },
                CurveSpan {
                    curve: second,
                    segment: 0,
                },
            ],
            [p0, p1, p2, p3],
        )
    }

    fn scene(document: &SketchDocument) -> EditorScene {
        #[allow(clippy::default_trait_access)]
        let session = geosolve_sketch::RetainedSketchDocumentSession::new(
            document.clone(),
            geosolve_sketch::DocumentSolveRequest::default(),
            Default::default(),
        )
        .expect("retained session");
        let accepted = session
            .accepted_state_for_current_input()
            .expect("accepted state");
        EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport"),
            0.5,
        )
        .expect("scene")
        .with_retained_session(&session)
        .expect("bound scene")
    }

    fn point_identity_branch_fixture() -> (RetainedEditorCoordinator, EditorScene, DesignPointId) {
        let mut document = SketchDocument::new(10.0).expect("document");
        let existing = document
            .add_point("existing", [0.0, 0.0])
            .expect("existing point");
        let support = document
            .add_point("support", [-2.0, 0.0])
            .expect("support point");
        document
            .add_curve(
                "profile incidence",
                CurveDefinition::Line {
                    start: existing,
                    end: support,
                    branch_direction: [-1.0, 0.0],
                },
            )
            .expect("profile line");
        document
            .add_constraint(
                "fix existing",
                DocumentConstraintDefinition::FixedPoint {
                    point: existing,
                    target: [0.0, 0.0],
                },
            )
            .expect("fixed existing point");
        let session = geosolve_sketch::RetainedSketchDocumentSession::new(
            document,
            geosolve_sketch::DocumentSolveRequest::default(),
            geosolve_sketch::SolverConfig::default(),
        )
        .expect("session");
        let accepted = session
            .accepted_state_for_current_input()
            .expect("accepted state");
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            accepted.design_identity(),
            accepted.document(),
            session.design_document(),
            Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport"),
            0.5,
        )
        .expect("scene")
        .with_retained_session(&session)
        .expect("bound scene");
        let mut coordinator =
            RetainedEditorCoordinator::new(session).expect("retained coordinator");
        let _ = coordinator
            .editor_mut()
            .set_geometry_visibility(native_geometry_policy().visibility);
        coordinator.editor_mut().activate_tool(EditorTool::Line);
        (coordinator, scene, existing)
    }

    fn inference_anchors(scene: &EditorScene, pointer: ScreenPoint) -> Vec<DraftReferenceAnchor> {
        match scene.draft_inference_anchors(pointer, DraftInferenceLimits::default()) {
            DraftInferenceAnchorCollection::Complete { anchors } => anchors,
            DraftInferenceAnchorCollection::ResourceLimited(evidence) => {
                panic!("ordinary test scene exceeded inference resources: {evidence:?}")
            }
        }
    }

    fn proposal_curve_fixture(
        label: &'static str,
        proposal: &ConstructionProposal,
        parameter: f64,
    ) -> (&'static str, SketchDocument, CurveSpan, f64) {
        let mut document = SketchDocument::new(10.0).expect("document");
        let result = proposal.apply(&mut document).expect("curve proposal");
        let span = document.curve_spans(result.curves[0]).expect("curve spans")[0];
        (label, document, span, parameter)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the native-family fixture table keeps every exact-evaluation contract directly comparable"
    )]
    fn native_point_on_curve_fixtures() -> Vec<(&'static str, SketchDocument, CurveSpan, f64)> {
        let mut fixtures = vec![
            proposal_curve_fixture(
                "line",
                &ConstructionProposal::Line {
                    start: ConstructionPoint::New([-3.0, -1.0]),
                    end: ConstructionPoint::New([4.0, 2.0]),
                },
                0.37,
            ),
            proposal_curve_fixture(
                "circle",
                &ConstructionProposal::Circle {
                    center: ConstructionPoint::New([0.0, 0.0]),
                    radius: 3.0,
                },
                1.1,
            ),
            proposal_curve_fixture(
                "circular arc",
                &ConstructionProposal::CounterClockwiseArc {
                    center: ConstructionPoint::New([0.0, 0.0]),
                    start: [3.0, 0.0],
                    end: [-1.0, 3.0],
                },
                0.37,
            ),
            proposal_curve_fixture(
                "quadratic Bezier",
                &ConstructionProposal::QuadraticBezier {
                    controls: [
                        ConstructionPoint::New([-3.0, -1.0]),
                        ConstructionPoint::New([-0.5, 4.0]),
                        ConstructionPoint::New([4.0, 0.5]),
                    ],
                },
                0.37,
            ),
            proposal_curve_fixture(
                "cubic Bezier",
                &ConstructionProposal::CubicBezier {
                    controls: [
                        ConstructionPoint::New([-4.0, -1.0]),
                        ConstructionPoint::New([-2.0, 5.0]),
                        ConstructionPoint::New([2.0, -4.0]),
                        ConstructionPoint::New([4.0, 1.0]),
                    ],
                },
                0.37,
            ),
            proposal_curve_fixture(
                "ellipse",
                &ConstructionProposal::Ellipse {
                    center: ConstructionPoint::New([0.0, 0.0]),
                    major_axis_point: ConstructionPoint::New([4.0, 1.0]),
                    minor_axis_ratio: 0.55,
                },
                1.1,
            ),
            proposal_curve_fixture(
                "elliptical arc",
                &ConstructionProposal::EllipticalArc {
                    center: ConstructionPoint::New([0.0, 0.0]),
                    major_axis_point: ConstructionPoint::New([4.0, 1.0]),
                    minor_axis_ratio: 0.55,
                    start_angle: -0.7,
                    end_angle: 2.2,
                    sweep: DocumentArcSweep::CounterClockwise,
                },
                0.37,
            ),
            proposal_curve_fixture(
                "rational quadratic conic",
                &ConstructionProposal::RationalQuadraticConic {
                    start: ConstructionPoint::New([-3.0, -1.0]),
                    weighted_middle: [0.0, 4.0],
                    middle_weight: 0.65,
                    end: ConstructionPoint::New([4.0, 0.5]),
                },
                0.37,
            ),
            proposal_curve_fixture(
                "parabola",
                &ConstructionProposal::Parabola {
                    vertex: ConstructionPoint::New([0.0, -1.0]),
                    focus: ConstructionPoint::New([0.5, 1.0]),
                    trim_start: -1.4,
                    trim_end: 1.7,
                },
                0.37,
            ),
            proposal_curve_fixture(
                "hyperbola",
                &ConstructionProposal::Hyperbola {
                    center: ConstructionPoint::New([0.0, 0.0]),
                    transverse_axis_point: ConstructionPoint::New([2.5, 0.5]),
                    semi_conjugate: 1.3,
                    branch: DocumentHyperbolaBranch::Positive,
                    trim_start: -1.1,
                    trim_end: 1.4,
                },
                0.37,
            ),
            proposal_curve_fixture(
                "NURBS",
                &ConstructionProposal::Nurbs {
                    controls: vec![
                        ConstructionPoint::New([-4.0, -1.0]),
                        ConstructionPoint::New([-2.0, 4.0]),
                        ConstructionPoint::New([2.0, -3.0]),
                        ConstructionPoint::New([4.0, 1.0]),
                    ],
                    options: NurbsConstructionOptions {
                        form: DocumentBSplineForm::Clamped,
                        degree: 3,
                        weights: vec![1.0, 0.7, 1.4, 1.0],
                        gauge_index: 0,
                    },
                },
                0.37,
            ),
        ];

        let mut bspline = SketchDocument::new(10.0).expect("B-spline document");
        let controls = [[-4.0, -1.0], [-2.0, 4.0], [2.0, -3.0], [4.0, 1.0]].map(|position| {
            bspline
                .add_point("B-spline control", position)
                .expect("control")
        });
        let curve = bspline
            .add_curve(
                "B-spline",
                CurveDefinition::BSpline {
                    form: DocumentBSplineForm::Clamped,
                    degree: 3,
                    controls: controls.to_vec(),
                    knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
                    span_ids: vec![41],
                    next_span_id: 42,
                },
            )
            .expect("B-spline");
        fixtures.insert(
            fixtures.len() - 1,
            ("B-spline", bspline, CurveSpan { curve, segment: 41 }, 0.37),
        );
        fixtures
    }

    fn construction_plan_effect(
        effects: &[EditorEffect],
    ) -> (ConstructionCommitToken, ConstructionCommitPlan) {
        effects
            .iter()
            .find_map(|effect| match effect {
                EditorEffect::CommitConstructionPlan { token, plan, .. } => {
                    Some((*token, plan.clone()))
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("construction plan effect expected, got {effects:?}"))
    }

    fn has_construction_commit(effects: &[EditorEffect]) -> bool {
        effects.iter().any(|effect| {
            matches!(
                effect,
                EditorEffect::CommitConstruction { .. }
                    | EditorEffect::CommitConstructionPlan { .. }
            )
        })
    }

    fn model_points_close(first: [f64; 2], second: [f64; 2]) -> bool {
        first
            .into_iter()
            .zip(second)
            .all(|(first, second)| (first - second).abs() < 1.0e-12)
    }

    fn acknowledge_planned_commit(editor: &mut ConstraintEditor, effects: &[EditorEffect]) {
        if let Some(token) = effects.iter().find_map(|effect| match effect {
            EditorEffect::CommitConstructionPlan { token, .. } => Some(*token),
            _ => None,
        }) {
            assert!(
                editor
                    .acknowledge_construction_commit(token, true)
                    .iter()
                    .any(|effect| matches!(effect, EditorEffect::ClearConstructionPreview))
            );
        }
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive table covers every valid and terminal construction stage"
    )]
    fn construction_stage_semantics_table_covers_every_editor_tool() {
        let point = |point_operand_ordinal| ConstructionStageSemantics {
            coordinate_role: ConstructionCoordinateRole::PointOperand,
            point_operand_ordinal: Some(point_operand_ordinal),
            directional_span: false,
            completed_span: None,
            reference_handoff: false,
        };
        let centered = |point_operand_ordinal| ConstructionStageSemantics {
            coordinate_role: ConstructionCoordinateRole::CenteredPointOperand {
                prospective_curve_index: 0,
            },
            point_operand_ordinal: Some(point_operand_ordinal),
            directional_span: false,
            completed_span: None,
            reference_handoff: false,
        };
        let coordinate_only = |coordinate_role| ConstructionStageSemantics {
            coordinate_role,
            point_operand_ordinal: None,
            directional_span: false,
            completed_span: None,
            reference_handoff: false,
        };
        let line_start = ConstructionStageSemantics {
            reference_handoff: true,
            ..point(0)
        };
        let line_end = ConstructionStageSemantics {
            directional_span: true,
            completed_span: Some(DraftSpanSlot::Created {
                curve_index: 0,
                segment: 0,
            }),
            ..point(1)
        };
        let polyline_stage =
            |stage_index: usize, segment: Option<u32>| ConstructionStageSemantics {
                directional_span: segment.is_some(),
                completed_span: segment.map(|segment| DraftSpanSlot::Created {
                    curve_index: 0,
                    segment,
                }),
                reference_handoff: true,
                ..point(stage_index)
            };
        let cases = vec![
            (EditorTool::Select, 0, None),
            (EditorTool::Point, 0, Some(point(0))),
            (EditorTool::Point, 1, None),
            (EditorTool::Line, 0, Some(line_start)),
            (EditorTool::Line, 1, Some(line_end)),
            (EditorTool::Line, 2, None),
            (EditorTool::Polyline, 0, Some(polyline_stage(0, None))),
            (EditorTool::Polyline, 1, Some(polyline_stage(1, Some(0)))),
            (EditorTool::Polyline, 2, Some(polyline_stage(2, Some(1)))),
            (EditorTool::Polyline, 3, Some(polyline_stage(3, Some(2)))),
            (
                EditorTool::Rectangle,
                0,
                Some(coordinate_only(ConstructionCoordinateRole::CoordinateOnly)),
            ),
            (
                EditorTool::Rectangle,
                1,
                Some(coordinate_only(ConstructionCoordinateRole::CoordinateOnly)),
            ),
            (EditorTool::Rectangle, 2, None),
            (EditorTool::Circle, 0, Some(centered(0))),
            (
                EditorTool::Circle,
                1,
                Some(coordinate_only(
                    ConstructionCoordinateRole::CircleCircumference,
                )),
            ),
            (EditorTool::Circle, 2, None),
            (EditorTool::CounterClockwiseArc, 0, Some(centered(0))),
            (
                EditorTool::CounterClockwiseArc,
                1,
                Some(coordinate_only(ConstructionCoordinateRole::CoordinateOnly)),
            ),
            (
                EditorTool::CounterClockwiseArc,
                2,
                Some(coordinate_only(ConstructionCoordinateRole::CoordinateOnly)),
            ),
            (EditorTool::CounterClockwiseArc, 3, None),
            (EditorTool::QuadraticBezier, 0, Some(point(0))),
            (EditorTool::QuadraticBezier, 1, Some(point(1))),
            (EditorTool::QuadraticBezier, 2, Some(point(2))),
            (EditorTool::QuadraticBezier, 3, None),
            (EditorTool::CubicBezier, 0, Some(point(0))),
            (EditorTool::CubicBezier, 1, Some(point(1))),
            (EditorTool::CubicBezier, 2, Some(point(2))),
            (EditorTool::CubicBezier, 3, Some(point(3))),
            (EditorTool::CubicBezier, 4, None),
            (EditorTool::Ellipse, 0, Some(centered(0))),
            (EditorTool::Ellipse, 1, Some(point(1))),
            (EditorTool::Ellipse, 2, None),
            (EditorTool::EllipticalArc, 0, Some(centered(0))),
            (EditorTool::EllipticalArc, 1, Some(point(1))),
            (
                EditorTool::EllipticalArc,
                2,
                Some(coordinate_only(ConstructionCoordinateRole::CoordinateOnly)),
            ),
            (
                EditorTool::EllipticalArc,
                3,
                Some(coordinate_only(ConstructionCoordinateRole::CoordinateOnly)),
            ),
            (EditorTool::EllipticalArc, 4, None),
            (EditorTool::RationalQuadraticConic, 0, Some(point(0))),
            (
                EditorTool::RationalQuadraticConic,
                1,
                Some(coordinate_only(ConstructionCoordinateRole::CoordinateOnly)),
            ),
            (EditorTool::RationalQuadraticConic, 2, Some(point(1))),
            (EditorTool::RationalQuadraticConic, 3, None),
            (EditorTool::Parabola, 0, Some(point(0))),
            (EditorTool::Parabola, 1, Some(point(1))),
            (EditorTool::Parabola, 2, None),
            (EditorTool::Hyperbola, 0, Some(centered(0))),
            (EditorTool::Hyperbola, 1, Some(point(1))),
            (EditorTool::Hyperbola, 2, None),
            (EditorTool::Nurbs, 0, Some(point(0))),
            (EditorTool::Nurbs, 1, Some(point(1))),
            (EditorTool::Nurbs, 2, Some(point(2))),
            (EditorTool::Nurbs, 3, Some(point(3))),
        ];
        let every_tool = [
            EditorTool::Select,
            EditorTool::Point,
            EditorTool::Line,
            EditorTool::Polyline,
            EditorTool::Rectangle,
            EditorTool::Circle,
            EditorTool::CounterClockwiseArc,
            EditorTool::QuadraticBezier,
            EditorTool::CubicBezier,
            EditorTool::Ellipse,
            EditorTool::EllipticalArc,
            EditorTool::RationalQuadraticConic,
            EditorTool::Parabola,
            EditorTool::Hyperbola,
            EditorTool::Nurbs,
        ];
        for tool in every_tool {
            assert!(
                cases.iter().any(|(candidate, _, _)| *candidate == tool),
                "missing stage-semantics coverage for {tool:?}"
            );
        }

        for (tool, stage_index, expected) in cases {
            let actual = construction_stage_semantics(tool, stage_index);
            assert_eq!(
                actual, expected,
                "unexpected complete semantics for {tool:?} stage {stage_index}"
            );
            assert_eq!(
                actual.map(|semantics| semantics.coordinate_role),
                expected.map(|semantics| semantics.coordinate_role),
                "unexpected coordinate role for {tool:?} stage {stage_index}"
            );
            let expected_subject = expected.and_then(|semantics| match semantics.coordinate_role {
                ConstructionCoordinateRole::PointOperand => {
                    Some(DraftInferenceSubject::PointOperand)
                }
                ConstructionCoordinateRole::CenteredPointOperand {
                    prospective_curve_index,
                } => Some(DraftInferenceSubject::CenteredPointOperand {
                    prospective_curve_index,
                }),
                ConstructionCoordinateRole::CircleCircumference => {
                    Some(DraftInferenceSubject::CircleCircumference)
                }
                ConstructionCoordinateRole::CoordinateOnly => None,
            });
            let actual_subject =
                actual.and_then(|semantics| semantics.coordinate_role.inference_subject());
            assert_eq!(
                actual_subject, expected_subject,
                "unexpected derived inference subject for {tool:?} stage {stage_index}"
            );
            assert_eq!(
                GeometryToolVariant::default_for_editor_tool(tool).and_then(|variant| {
                    construction_stage_semantics_for(false, tool, variant, stage_index)
                        .and_then(|semantics| semantics.coordinate_role.inference_subject())
                }),
                expected_subject,
                "unexpected inference lookup for {tool:?} stage {stage_index}"
            );
            let expected_centered_curve = expected.and_then(|semantics| {
                if let ConstructionCoordinateRole::CenteredPointOperand {
                    prospective_curve_index,
                } = semantics.coordinate_role
                {
                    Some(prospective_curve_index)
                } else {
                    None
                }
            });
            assert_eq!(
                actual_subject.and_then(DraftInferenceSubject::prospective_centered_curve_index),
                expected_centered_curve,
                "unexpected prospective centered curve for {tool:?} stage {stage_index}"
            );
            assert_eq!(
                actual.and_then(|semantics| semantics.point_operand_ordinal),
                expected.and_then(|semantics| semantics.point_operand_ordinal),
                "unexpected point ordinal for {tool:?} stage {stage_index}"
            );
            assert_eq!(
                actual.map(|semantics| semantics.directional_span),
                expected.map(|semantics| semantics.directional_span),
                "unexpected directional ownership for {tool:?} stage {stage_index}"
            );
            assert_eq!(
                actual.and_then(|semantics| semantics.completed_span),
                expected.and_then(|semantics| semantics.completed_span),
                "unexpected completed span for {tool:?} stage {stage_index}"
            );
            assert_eq!(
                actual.map(|semantics| semantics.reference_handoff),
                expected.map(|semantics| semantics.reference_handoff),
                "unexpected reference handoff for {tool:?} stage {stage_index}"
            );
        }

        if let Ok(last_representable_stage) = usize::try_from(u64::from(u32::MAX) + 1) {
            assert_eq!(
                construction_stage_semantics(EditorTool::Polyline, last_representable_stage)
                    .and_then(|semantics| semantics.completed_span),
                Some(DraftSpanSlot::Created {
                    curve_index: 0,
                    segment: u32::MAX,
                })
            );
            if let Some(first_unrepresentable_stage) = last_representable_stage.checked_add(1) {
                let semantics =
                    construction_stage_semantics(EditorTool::Polyline, first_unrepresentable_stage)
                        .expect("an arbitrarily long polyline still has stage semantics");
                assert_eq!(
                    semantics.coordinate_role,
                    ConstructionCoordinateRole::PointOperand
                );
                assert_eq!(
                    semantics.point_operand_ordinal,
                    Some(first_unrepresentable_stage)
                );
                assert!(semantics.directional_span);
                assert_eq!(semantics.completed_span, None);
                assert!(semantics.reference_handoff);
            }
        }
    }

    #[test]
    fn advanced_positional_inference_keeps_interleaved_slots_and_mixed_operands_exact() {
        let (document, lines, points) = line_document();
        let scene = scene(&document);
        let click = |editor: &mut ConstraintEditor, pointer_id, model: [f64; 2]| {
            let screen = scene.viewport.model_to_screen(model);
            editor.pointer_down(
                &scene,
                pointer(pointer_id, screen.x, screen.y, Modifiers::default()),
            )
        };

        let mut conic = ConstraintEditor::default();
        conic.activate_tool(EditorTool::RationalQuadraticConic);
        click(&mut conic, 61, [-4.0, 1.0]);
        click(&mut conic, 61, [0.0, 4.0]);
        assert_eq!(
            conic
                .draft
                .as_ref()
                .expect("interleaved conic draft")
                .confirmed_inference
                .len(),
            1,
            "the weighted-middle coordinate stage must not acquire positional inference"
        );
        let conic_effects = click(&mut conic, 61, [0.0, 1.0]);
        let (_, conic_plan) = construction_plan_effect(&conic_effects);
        assert!(matches!(
            conic_plan.proposal,
            ConstructionProposal::RationalQuadraticConic {
                start: ConstructionPoint::Existing { id, .. },
                weighted_middle,
                end: ConstructionPoint::New(end),
                ..
            } if id == points[0]
                && model_points_close(weighted_middle, [0.0, 4.0])
                && model_points_close(end, [0.0, 1.0])
        ));
        assert!(matches!(
            conic_plan.relations.as_slice(),
            [InferredRelation::Midpoint {
                point: DraftPointSlot::Created { point_index: 0 },
                line: DraftSpanSlot::Existing(line),
            }] if *line == lines[0]
        ));
        let mut conic_document = document.clone();
        let conic_result = conic_plan
            .apply(&mut conic_document)
            .expect("interleaved conic plan");
        assert_eq!(conic_result.construction.points.len(), 1);
        assert!(matches!(
            conic_document
                .constraint(conic_result.constraints[0].constraint)
                .expect("conic midpoint")
                .definition,
            DocumentConstraintDefinition::Midpoint { point, line }
                if point == conic_result.construction.points[0] && line == lines[0]
        ));

        let mut cubic = ConstraintEditor::default();
        cubic.activate_tool(EditorTool::CubicBezier);
        click(&mut cubic, 62, [-4.0, 1.0]);
        click(&mut cubic, 62, [-2.0, 4.0]);
        click(&mut cubic, 62, [0.0, 1.0]);
        let cubic_effects = click(&mut cubic, 62, [4.0, 1.0]);
        let (_, cubic_plan) = construction_plan_effect(&cubic_effects);
        assert!(matches!(
            &cubic_plan.proposal,
            ConstructionProposal::CubicBezier { controls }
                if matches!(controls[0], ConstructionPoint::Existing { id, .. } if id == points[0])
                    && matches!(controls[1], ConstructionPoint::New(position) if model_points_close(position, [-2.0, 4.0]))
                    && matches!(controls[2], ConstructionPoint::New(position) if model_points_close(position, [0.0, 1.0]))
                    && matches!(controls[3], ConstructionPoint::Existing { id, .. } if id == points[1])
        ));
        assert!(matches!(
            cubic_plan.relations.as_slice(),
            [InferredRelation::Midpoint {
                point: DraftPointSlot::Created { point_index: 1 },
                line: DraftSpanSlot::Existing(line),
            }] if *line == lines[0]
        ));
        let mut cubic_document = document.clone();
        let cubic_result = cubic_plan
            .apply(&mut cubic_document)
            .expect("mixed-control cubic plan");
        assert_eq!(cubic_result.construction.points.len(), 2);
        assert!(matches!(
            cubic_document
                .constraint(cubic_result.constraints[0].constraint)
                .expect("cubic midpoint")
                .definition,
            DocumentConstraintDefinition::Midpoint { point, line }
                if point == cubic_result.construction.points[1] && line == lines[0]
        ));
    }

    #[test]
    fn scene_anchors_retain_exact_bounded_and_periodic_contact_topology() {
        let (document, lines, _) = line_document();
        let scene = scene(&document);
        let start = scene.viewport.model_to_screen([-4.0, 1.0]);
        let anchors = inference_anchors(&scene, start);
        assert!(anchors.iter().any(|anchor| matches!(
            anchor,
            DraftReferenceAnchor::AffineSupport { contact, .. }
                if contact.span == lines[0]
                    && contact.parameter == 0.0
                    && contact.winding == 0
                    && contact.neighborhood == ContactNeighborhood::Start
        )));
        let end = scene.viewport.model_to_screen([4.0, 1.0]);
        assert!(inference_anchors(&scene, end).iter().any(|anchor| matches!(
            anchor,
                DraftReferenceAnchor::AffineSupport { contact, .. }
                if contact.span == lines[0]
                    && (contact.parameter - 1.0).abs() < 1.0e-12
                    && contact.neighborhood == ContactNeighborhood::End
        )));

        let period = std::f64::consts::TAU;
        let (_, periodic_document, periodic_span, _) = native_point_on_curve_fixtures()
            .into_iter()
            .find(|(family, ..)| *family == "circle")
            .expect("periodic circle fixture");
        let contact = draft_curve_contact(
            &periodic_document,
            periodic_span,
            ContactDomain::Periodic { period },
            period.mul_add(2.0, 0.25),
        )
        .expect("periodic contact");
        assert_eq!(contact.parameter.to_bits(), 0.25f64.to_bits());
        assert_eq!(contact.winding, 2);
        let seam = draft_curve_contact(
            &periodic_document,
            periodic_span,
            ContactDomain::Periodic { period },
            period,
        )
        .expect("periodic seam");
        assert_eq!(seam.parameter.to_bits(), 0.0f64.to_bits());
        assert_eq!(seam.winding, 1);
        let negative = draft_curve_contact(
            &periodic_document,
            periodic_span,
            ContactDomain::Periodic { period },
            -0.25,
        )
        .expect("negative winding");
        assert!((negative.parameter - (period - 0.25)).abs() < 1.0e-12);
        assert_eq!(negative.winding, -1);
    }

    #[test]
    fn scene_anchor_resource_limit_fails_closed_without_a_partial_prefix() {
        let (document, _, _) = line_document();
        let line_scene = scene(&document);
        let pointer_position = line_scene.viewport.model_to_screen([0.0, 1.0]);
        let mut policy = DraftInferencePolicy::default();
        policy.limits.max_scene_anchors = 7;
        let evidence = DraftInferenceSceneLimit {
            resource: DraftInferenceSceneResource::Anchors,
            required: 8,
            limit: 7,
        };
        assert_eq!(
            line_scene.draft_inference_anchors(pointer_position, policy.limits),
            DraftInferenceAnchorCollection::ResourceLimited(evidence)
        );
        let segment_limits = DraftInferenceLimits {
            max_scene_curve_segments: 1,
            ..DraftInferenceLimits::default()
        };
        assert_eq!(
            line_scene.draft_inference_anchors(pointer_position, segment_limits),
            DraftInferenceAnchorCollection::ResourceLimited(DraftInferenceSceneLimit {
                resource: DraftInferenceSceneResource::CurveSegments,
                required: 2,
                limit: 1,
            })
        );

        let mut editor = ConstraintEditor::default();
        editor
            .set_draft_inference_policy(policy)
            .expect("bounded policy");
        let resolution = editor
            .resolve_draft_inference(
                &line_scene,
                pointer_position,
                DraftInferenceInput::default(),
                EditorTool::Point,
                0,
                None,
            )
            .expect("typed scene limit")
            .expect("point stage resolution");
        assert_eq!(resolution.status, DraftInferenceStatus::ResourceLimited);
        assert_eq!(
            resolution.completeness,
            DraftInferenceCompleteness::SceneLimit(evidence)
        );
        assert!(resolution.candidates.is_empty());
        assert!(resolution.guides.is_empty());

        editor.activate_tool(EditorTool::Point);
        let effects = editor.pointer_down(
            &line_scene,
            pointer(
                91,
                pointer_position.x,
                pointer_position.y,
                Modifiers::default(),
            ),
        );
        assert!(editor.draft.is_none());
        assert!(!has_construction_commit(&effects));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            EditorEffect::DraftInferenceChanged(Some(resolution))
                if resolution.status == DraftInferenceStatus::ResourceLimited
                    && resolution.completeness
                        == DraftInferenceCompleteness::SceneLimit(evidence)
        )));
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one collection contract covers subject routing, shared exact bounds, and suppression bypass"
    )]
    fn subject_specific_scene_collection_is_complete_bounded_and_traversal_minimal() {
        let (document, _, _) = line_document();
        let line_scene = scene(&document);
        let pointer_position = line_scene.viewport.model_to_screen([0.0, 1.0]);
        let segment_limits = DraftInferenceLimits {
            max_scene_curve_segments: 0,
            ..DraftInferenceLimits::default()
        };
        assert!(matches!(
            line_scene.draft_inference_scene_inputs(
                pointer_position,
                DraftInferenceSubject::CircleCircumference,
                segment_limits,
            ),
            DraftInferenceSceneInputCollection::Complete(DraftInferenceSceneInputs {
                ref anchors,
                ref semantic_centers,
            }) if anchors.len() == 4
                && semantic_centers.is_empty()
                && anchors.iter().all(|anchor| matches!(
                    anchor,
                    DraftReferenceAnchor::PersistentPoint { .. }
                ))
        ));
        assert!(matches!(
            line_scene.draft_inference_scene_inputs(
                pointer_position,
                DraftInferenceSubject::PointOperand,
                segment_limits,
            ),
            DraftInferenceSceneInputCollection::ResourceLimited(DraftInferenceSceneLimit {
                resource: DraftInferenceSceneResource::CurveSegments,
                ..
            })
        ));

        let mut centered_document = SketchDocument::new(10.0).expect("document");
        for (label, center, radius_value) in
            [("first", [-2.0, 0.0], 1.0), ("second", [2.0, 0.0], 1.5)]
        {
            let center = centered_document.add_point(label, center).expect("center");
            let radius = centered_document
                .add_scalar(
                    label,
                    radius_value,
                    ScalarUnit::Length,
                    ScalarDomain::Positive,
                )
                .expect("radius");
            centered_document
                .add_curve(label, CurveDefinition::Circle { center, radius })
                .expect("circle");
        }
        let centered_scene = scene(&centered_document);
        let pointer_position = centered_scene.viewport.model_to_screen([0.0, 0.0]);
        let exact = DraftInferenceLimits {
            max_scene_anchors: 7,
            ..DraftInferenceLimits::default()
        };
        let exact_inputs = centered_scene.draft_inference_scene_inputs(
            pointer_position,
            DraftInferenceSubject::CenteredPointOperand {
                prospective_curve_index: 0,
            },
            exact,
        );
        assert!(
            matches!(
                exact_inputs,
                DraftInferenceSceneInputCollection::Complete(DraftInferenceSceneInputs {
                    ref anchors,
                    ref semantic_centers,
                }) if anchors.len() == 5 && semantic_centers.len() == 2
            ),
            "unexpected exact centered inputs: {exact_inputs:?}"
        );
        let limited = DraftInferenceLimits {
            max_scene_anchors: 6,
            ..DraftInferenceLimits::default()
        };
        assert_eq!(
            centered_scene.draft_inference_scene_inputs(
                pointer_position,
                DraftInferenceSubject::CenteredPointOperand {
                    prospective_curve_index: 0,
                },
                limited,
            ),
            DraftInferenceSceneInputCollection::ResourceLimited(DraftInferenceSceneLimit {
                resource: DraftInferenceSceneResource::Anchors,
                required: 7,
                limit: 6,
            })
        );

        let mut editor = ConstraintEditor::default();
        let mut policy = DraftInferencePolicy::default();
        policy.limits.max_scene_anchors = 1;
        editor.set_draft_inference_policy(policy).expect("policy");
        let suppressed = editor
            .resolve_draft_inference(
                &centered_scene,
                pointer_position,
                DraftInferenceInput {
                    suppressed: true,
                    preferred_candidate: None,
                },
                EditorTool::Circle,
                0,
                None,
            )
            .expect("suppressed collection bypass")
            .expect("center stage");
        assert_eq!(suppressed.status, DraftInferenceStatus::Suppressed);
        assert_eq!(
            suppressed.completeness,
            DraftInferenceCompleteness::Complete
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one tool-agnostic matrix proves the centered-operand law across every centered construction"
    )]
    fn every_centered_tool_defaults_to_concentric_and_can_explicitly_reuse_identity() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let center = document.add_point("center", [0.0, 0.0]).expect("center");
        let radius = document
            .add_scalar("radius", 2.0, ScalarUnit::Length, ScalarDomain::Positive)
            .expect("radius");
        let reference = document
            .add_curve("reference", CurveDefinition::Circle { center, radius })
            .expect("circle");
        let scene = scene(&document);
        let center_screen = scene.viewport.model_to_screen([0.0, 0.0]);
        let tools = [
            (EditorTool::Circle, [3.0, 0.0]),
            (EditorTool::CounterClockwiseArc, [3.0, 0.0]),
            (EditorTool::Ellipse, [3.0, 0.5]),
            (EditorTool::EllipticalArc, [3.0, 0.5]),
            (EditorTool::Hyperbola, [3.0, 0.5]),
        ];
        for (ordinal, (tool, second_position)) in tools.into_iter().enumerate() {
            let mut editor = ConstraintEditor::default();
            editor.activate_tool(tool);
            let near_center = scene.viewport.model_to_screen([0.05, 0.02]);
            let pointer_id = 300 + u64::try_from(ordinal).expect("ordinal");
            let preview = editor.pointer_move(
                &scene,
                pointer(
                    pointer_id,
                    near_center.x,
                    near_center.y,
                    Modifiers::default(),
                ),
            );
            assert!(preview.iter().any(|effect| matches!(
                effect,
                EditorEffect::DraftInferenceChanged(Some(DraftInferenceResolution {
                    status: DraftInferenceStatus::Resolved { candidate },
                    candidates,
                    ..
                })) if candidates.iter().any(|candidate_value| {
                    candidate_value.id == *candidate
                        && matches!(
                            candidate_value.relations.as_slice(),
                            [DraftInferenceRelation::Concentric {
                                reference: inferred,
                                prospective_curve_index: 0,
                            }] if *inferred == reference
                        )
                })
            )));
            let resolution = editor
                .draft_inference_resolution()
                .expect("centered resolution");
            let DraftInferenceStatus::Resolved {
                candidate: candidate_id,
            } = resolution.status
            else {
                panic!("resolved centered candidate");
            };
            let selected_candidate = resolution
                .candidates
                .iter()
                .find(|candidate| candidate.id == candidate_id)
                .expect("selected centered candidate")
                .clone();
            assert!(
                selected_candidate
                    .guides
                    .iter()
                    .all(|guide| guide.id.candidate == Some(candidate_id))
            );
            editor.pointer_down(
                &scene,
                pointer(
                    pointer_id,
                    near_center.x,
                    near_center.y,
                    Modifiers::default(),
                ),
            );
            let first = editor.draft.as_ref().expect("centered draft");
            assert!(
                matches!(
                    first.points[0],
                    ConstructionPoint::New(position) if model_points_close(position, [0.0, 0.0])
                ),
                "unexpected {tool:?} center operand: {:?}",
                first.points[0]
            );
            assert_eq!(first.confirmed_inference[0].candidate_id, candidate_id);
            assert_eq!(
                first.confirmed_inference[0].relations,
                selected_candidate.relations
            );
            assert_eq!(
                first.confirmed_inference[0].references,
                selected_candidate.references
            );
            assert!(matches!(
                first.confirmed_inference[0].relations.as_slice(),
                [DraftInferenceRelation::Concentric {
                    reference: inferred,
                    prospective_curve_index: 0,
                }] if *inferred == reference
            ));

            let second = scene.viewport.model_to_screen(second_position);
            let mut effects = editor.pointer_down_with_draft_inference(
                &scene,
                pointer(pointer_id, second.x, second.y, Modifiers::default()),
                DraftInferenceInput {
                    suppressed: true,
                    preferred_candidate: None,
                },
            );
            match tool {
                EditorTool::CounterClockwiseArc => {
                    let third = scene.viewport.model_to_screen([0.0, 3.0]);
                    effects = editor.pointer_down_with_draft_inference(
                        &scene,
                        pointer(pointer_id, third.x, third.y, Modifiers::default()),
                        DraftInferenceInput {
                            suppressed: true,
                            preferred_candidate: None,
                        },
                    );
                }
                EditorTool::EllipticalArc => {
                    for trim in [[2.0, 2.0], [-1.0, 1.5]] {
                        let trim = scene.viewport.model_to_screen(trim);
                        effects = editor.pointer_down_with_draft_inference(
                            &scene,
                            pointer(pointer_id, trim.x, trim.y, Modifiers::default()),
                            DraftInferenceInput {
                                suppressed: true,
                                preferred_candidate: None,
                            },
                        );
                    }
                }
                _ => {}
            }
            let (_, plan) = construction_plan_effect(&effects);
            assert!(matches!(
                plan.relations.as_slice(),
                [InferredRelation::Concentric {
                    first: DraftCurveSlot::Created { curve_index: 0 },
                    second: DraftCurveSlot::Existing(existing),
                }] if *existing == reference
            ));
            let mut candidate = document.clone();
            let result = plan.apply(&mut candidate).expect("atomic concentric plan");
            assert_eq!(result.construction.curves.len(), 1);
            assert_eq!(result.constraints.len(), 1);
            assert!(matches!(
                candidate
                    .constraint(result.constraints[0].constraint)
                    .expect("concentric constraint")
                    .definition,
                DocumentConstraintDefinition::Concentric { first, second }
                    if first.curve == result.construction.curves[0]
                        && second.curve == reference
            ));

            let mut identity_editor = ConstraintEditor::default();
            let identity_policy = DraftInferencePolicy {
                concentric: DraftInferenceBehavior {
                    show_guides: false,
                    adjust_coordinates: false,
                    persist_constraint: false,
                },
                ..DraftInferencePolicy::default()
            };
            identity_editor
                .set_draft_inference_policy(identity_policy)
                .expect("identity policy");
            identity_editor.activate_tool(tool);
            identity_editor.pointer_down(
                &scene,
                pointer(
                    200 + u64::try_from(ordinal).expect("ordinal"),
                    center_screen.x,
                    center_screen.y,
                    Modifiers::default(),
                ),
            );
            assert!(matches!(
                identity_editor
                    .draft
                    .as_ref()
                    .expect("centered identity draft")
                    .points[0],
                ConstructionPoint::Existing { id, .. } if id == center
            ));
        }
    }

    #[test]
    fn centered_circle_preserves_midpoint_and_generic_point_on_curve_authoring() {
        let (line_document, lines, _) = line_document();
        let line_scene = scene(&line_document);
        let midpoint = [0.0, 1.0];
        let mut midpoint_editor = ConstraintEditor::default();
        midpoint_editor.activate_tool(EditorTool::Circle);
        let center = line_scene.viewport.model_to_screen(midpoint);
        midpoint_editor.pointer_down(
            &line_scene,
            pointer(410, center.x, center.y, Modifiers::default()),
        );
        let rim = line_scene.viewport.model_to_screen([1.0, 1.0]);
        let effects = midpoint_editor.pointer_down_with_draft_inference(
            &line_scene,
            pointer(410, rim.x, rim.y, Modifiers::default()),
            DraftInferenceInput {
                suppressed: true,
                preferred_candidate: None,
            },
        );
        let (_, plan) = construction_plan_effect(&effects);
        assert!(matches!(
            plan.relations.as_slice(),
            [InferredRelation::Midpoint {
                point: DraftPointSlot::Created { point_index: 0 },
                line: DraftSpanSlot::Existing(line),
            }] if *line == lines[0]
        ));
        plan.apply(&mut line_document.clone())
            .expect("center-at-midpoint plan");

        let (_, curve_document, curve, parameter) = native_point_on_curve_fixtures()
            .into_iter()
            .find(|(family, ..)| *family == "cubic Bezier")
            .expect("cubic fixture");
        let curve_scene = scene(&curve_document);
        let jet = curve_document
            .evaluate_curve_jet(curve, parameter)
            .expect("curve point");
        let curve_center = [jet.position.x, jet.position.y];
        let center = curve_scene.viewport.model_to_screen(curve_center);
        let mut curve_editor = ConstraintEditor::default();
        curve_editor.activate_tool(EditorTool::Circle);
        curve_editor.pointer_down(
            &curve_scene,
            pointer(411, center.x, center.y, Modifiers::default()),
        );
        let rim = curve_scene
            .viewport
            .model_to_screen([curve_center[0] + 1.0, curve_center[1]]);
        let effects = curve_editor.pointer_down_with_draft_inference(
            &curve_scene,
            pointer(411, rim.x, rim.y, Modifiers::default()),
            DraftInferenceInput {
                suppressed: true,
                preferred_candidate: None,
            },
        );
        let (_, plan) = construction_plan_effect(&effects);
        assert!(matches!(
            plan.relations.as_slice(),
            [InferredRelation::PointOnCurve {
                point: DraftPointSlot::Created { point_index: 0 },
                contact: DraftContactDescriptor {
                    span: DraftSpanSlot::Existing(span),
                    ..
                },
            }] if *span == curve
        ));
        plan.apply(&mut curve_document.clone())
            .expect("center-on-curve plan");
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "both nonlinear contact branches and stale candidate selection share one fixture"
    )]
    fn nonlinear_self_intersection_preserves_both_contact_branches() {
        let controls = [
            ConstructionPoint::New([0.0, 0.0]),
            ConstructionPoint::New([2.0, 3.0]),
            ConstructionPoint::New([-2.0, 3.0]),
            ConstructionPoint::New([84.0 / 79.0, 0.0]),
        ];
        let (_, mut document, span, _) = proposal_curve_fixture(
            "self-intersecting cubic",
            &ConstructionProposal::CubicBezier { controls },
            0.0,
        );
        document
            .add_point("distinct stale-preference target", [5.0, 5.0])
            .expect("point");
        let scene = scene(&document);
        let first_parameter = 0.089_385_032_953_265_66;
        let second_parameter = 1.0 - first_parameter;
        let crossing = document
            .evaluate_curve_jet(span, first_parameter)
            .expect("first crossing branch")
            .position;
        let pointer_position = scene.viewport.model_to_screen([crossing.x, crossing.y]);
        let branches = inference_anchors(&scene, pointer_position)
            .into_iter()
            .filter_map(|anchor| match anchor {
                DraftReferenceAnchor::CurvePoint {
                    contact,
                    branch_candidate,
                    ..
                } if contact.span == span => Some((contact, branch_candidate)),
                DraftReferenceAnchor::PersistentPoint { .. }
                | DraftReferenceAnchor::Midpoint { .. }
                | DraftReferenceAnchor::CurvePoint { .. }
                | DraftReferenceAnchor::AffineSupport { .. } => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(branches.len(), 2);
        assert!((branches[0].0.parameter - first_parameter).abs() <= 1.0e-10);
        assert!((branches[1].0.parameter - second_parameter).abs() <= 1.0e-10);
        assert_eq!(branches[0].1.get(), 0);
        assert_eq!(branches[1].1.get(), 1);

        let mut editor = ConstraintEditor::default();
        editor.activate_tool(EditorTool::Point);
        let resolution = editor
            .resolve_draft_inference(
                &scene,
                pointer_position,
                DraftInferenceInput::default(),
                EditorTool::Point,
                0,
                None,
            )
            .expect("self-intersection resolution")
            .expect("point stage resolution");
        let DraftInferenceStatus::Ambiguous { candidates } = &resolution.status else {
            panic!(
                "self-intersection must remain explicit, got {:?}",
                resolution.status
            );
        };
        assert_eq!(candidates.len(), 2);
        assert_eq!(resolution.candidates.len(), 2);
        assert!(resolution.guides.iter().all(|guide| {
            guide
                .id
                .candidate
                .is_none_or(|candidate| candidates.contains(&candidate))
        }));
        let mut forged_resolution = resolution.clone();
        forged_resolution.status = DraftInferenceStatus::Resolved {
            candidate: candidates[0],
        };
        let wrong_candidate = forged_resolution
            .candidates
            .iter()
            .find(|candidate| candidate.id == candidates[1])
            .expect("other ambiguous candidate")
            .clone();
        assert!(matches!(
            confirmed_draft_inference(&forged_resolution, wrong_candidate, 0),
            Err(DraftInferenceError::InvalidFrame)
        ));

        let stale_preferred = candidates[0];
        let endpoint = scene.viewport.model_to_screen([5.0, 5.0]);
        let stale = editor.pointer_down_with_draft_inference(
            &scene,
            pointer(93, endpoint.x, endpoint.y, Modifiers::default()),
            DraftInferenceInput {
                suppressed: false,
                preferred_candidate: Some(stale_preferred),
            },
        );
        assert!(editor.draft.is_none());
        assert!(editor.pending_construction_commit_token().is_none());
        assert!(!has_construction_commit(&stale));
        assert!(stale.iter().any(|effect| matches!(
            effect,
            EditorEffect::DraftInferenceChanged(Some(DraftInferenceResolution {
                status: DraftInferenceStatus::StalePreferredCandidate { preferred },
                ..
            })) if *preferred == stale_preferred
        )));

        let mut click_editor = ConstraintEditor::default();
        click_editor.activate_tool(EditorTool::Point);
        let effects = click_editor.pointer_down(
            &scene,
            pointer(
                92,
                pointer_position.x,
                pointer_position.y,
                Modifiers::default(),
            ),
        );
        assert!(click_editor.draft.is_none());
        assert!(!has_construction_commit(&effects));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            EditorEffect::DraftInferenceChanged(Some(resolution))
                if matches!(resolution.status, DraftInferenceStatus::Ambiguous { .. })
        )));
    }

    #[test]
    fn scene_inference_is_invariant_to_sketch_model_scale() {
        let resolve = |model_scale: f64| {
            let mut document = SketchDocument::new(model_scale).expect("document");
            let start = document.add_point("start", [-4.0, 0.0]).expect("point");
            let end = document.add_point("end", [4.0, 0.0]).expect("point");
            document
                .add_curve(
                    "reference",
                    CurveDefinition::Line {
                        start,
                        end,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .expect("line");
            let scene = scene(&document);
            let pointer = scene.viewport.model_to_screen([0.0, 0.0]);
            let resolution = ConstraintEditor::default()
                .resolve_draft_inference(
                    &scene,
                    pointer,
                    DraftInferenceInput::default(),
                    EditorTool::Point,
                    0,
                    None,
                )
                .expect("inference")
                .expect("point-stage resolution");
            let DraftInferenceStatus::Resolved { candidate: winner } = resolution.status else {
                panic!("expected resolved midpoint inference");
            };
            let summaries = resolution
                .candidates
                .iter()
                .map(|candidate| {
                    let [relation] = candidate.relations.as_slice() else {
                        panic!("expected one positional relation");
                    };
                    let (family, contact) = match relation {
                        DraftInferenceRelation::Midpoint { .. } => ("midpoint", None),
                        DraftInferenceRelation::PointOnCurve { contact } => (
                            "point-on-curve",
                            Some((
                                contact.domain,
                                contact.parameter.to_bits(),
                                contact.winding,
                                contact.neighborhood,
                            )),
                        ),
                        DraftInferenceRelation::CoincidentWithOrigin
                        | DraftInferenceRelation::PointOnDatumAxis { .. }
                        | DraftInferenceRelation::PointIdentity { .. }
                        | DraftInferenceRelation::PointOnCreatedCurve { .. }
                        | DraftInferenceRelation::Horizontal
                        | DraftInferenceRelation::Vertical
                        | DraftInferenceRelation::Parallel { .. }
                        | DraftInferenceRelation::Perpendicular { .. }
                        | DraftInferenceRelation::HorizontalPoints { .. }
                        | DraftInferenceRelation::VerticalPoints { .. }
                        | DraftInferenceRelation::HorizontalPointToMidpoint { .. }
                        | DraftInferenceRelation::VerticalPointToMidpoint { .. }
                        | DraftInferenceRelation::Concentric { .. }
                        | DraftInferenceRelation::Collinear { .. } => {
                            panic!("unexpected midpoint-scene relation")
                        }
                    };
                    (
                        candidate.id == winner,
                        family,
                        contact,
                        candidate.ranking,
                        candidate.adjusted_model_position.map(f64::to_bits),
                    )
                })
                .collect::<Vec<_>>();
            (resolution.completeness, summaries)
        };

        let baseline = resolve(1.0);
        for model_scale in [1.0e-6, 1.0e6] {
            assert_eq!(resolve(model_scale), baseline);
        }
    }

    #[test]
    fn standalone_point_reuses_identity_and_point_on_curve_keeps_contact_metadata() {
        let (document, lines, points) = line_document();
        let scene = scene(&document);
        let mut editor = ConstraintEditor::default();
        editor.activate_tool(EditorTool::Point);
        let endpoint = scene.viewport.model_to_screen([-4.0, 1.0]);
        let endpoint_effects = editor.pointer_down(
            &scene,
            pointer(1, endpoint.x, endpoint.y, Modifiers::default()),
        );
        assert!(!has_construction_commit(&endpoint_effects));
        assert!(editor.pending_construction_commit_token().is_none());
        assert!(
            inference_anchors(&scene, endpoint)
                .iter()
                .any(|anchor| matches!(
                    anchor,
                    DraftReferenceAnchor::PersistentPoint { point, .. } if *point == points[0]
                ))
        );

        let quarter = scene.viewport.model_to_screen([-2.0, 1.0]);
        let curve_effects = editor.pointer_down(
            &scene,
            pointer(1, quarter.x, quarter.y, Modifiers::default()),
        );
        let (_, plan) = construction_plan_effect(&curve_effects);
        let expected_neighborhood = document
            .picked_contact_neighborhood(lines[0], 0.25)
            .expect("line contact neighborhood");
        assert!(matches!(
            plan.proposal,
            ConstructionProposal::Point {
                point: ConstructionPoint::New(position)
            } if (position[0] + 2.0).abs() < 1.0e-12
                && (position[1] - 1.0).abs() < 1.0e-12
        ));
        assert!(matches!(
            plan.relations.as_slice(),
            [InferredRelation::PointOnCurve {
                point: DraftPointSlot::Created { point_index: 0 },
                contact: DraftContactDescriptor {
                    span: DraftSpanSlot::Existing(span),
                    domain: ContactDomain::Bounded { lower: 0.0, upper: 1.0 },
                    parameter,
                    winding: 0,
                    neighborhood,
                },
            }] if *span == lines[0]
                && (*parameter - 0.25).abs() < 1.0e-12
                && *neighborhood == expected_neighborhood
        ));
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one regression keeps adjusted preview, exact lowering, contact metadata, and allocation ownership contiguous"
    )]
    fn circle_circumference_snaps_through_an_existing_point_without_a_hidden_rim_point() {
        let (document, _, points) = line_document();
        let scene = scene(&document);
        let center_position = [0.0, 0.0];
        let target_position = [4.0, 1.0];
        let expected_radius = 17.0_f64.sqrt();
        let expected_parameter = 1.0_f64.atan2(4.0).rem_euclid(std::f64::consts::TAU);
        let mut editor = ConstraintEditor::default();
        editor.activate_tool(EditorTool::Circle);

        let center = scene.viewport.model_to_screen(center_position);
        editor.pointer_down(
            &scene,
            pointer(82, center.x, center.y, Modifiers::default()),
        );
        let near_target = scene.viewport.model_to_screen([4.05, 1.03]);
        let preview = editor.pointer_move(
            &scene,
            pointer(82, near_target.x, near_target.y, Modifiers::default()),
        );
        assert!(preview.iter().any(|effect| matches!(
            effect,
            EditorEffect::PreviewConstruction(ConstructionPreview::Complete {
                proposal: ConstructionProposal::Circle { radius, .. },
                geometry: ConstructionPreviewGeometry::Circle {
                    center,
                    radius: visible_radius,
                },
            }) if model_points_close(*center, center_position)
                && (*radius - expected_radius).abs() < 1.0e-12
                && (*visible_radius - expected_radius).abs() < 1.0e-12
        )));
        let resolution = editor
            .draft_inference_resolution()
            .expect("circumference inference resolution");
        assert!(model_points_close(
            resolution.adjusted_model_position,
            target_position
        ));
        let candidate = resolved_draft_inference_candidate(resolution)
            .expect("resolved circle-through-point candidate");
        assert!(matches!(
            candidate.relations.as_slice(),
            [DraftInferenceRelation::PointOnCreatedCurve { point }] if *point == points[1]
        ));
        let candidate = candidate.clone();
        assert!(
            candidate
                .guides
                .iter()
                .all(|guide| guide.id.candidate == Some(candidate.id))
        );
        let prior = editor.draft.clone();
        let resolved = editor
            .resolve_draft_stage(
                &scene,
                near_target,
                DraftInferenceInput::default(),
                EditorTool::Circle,
                1,
                prior.as_ref(),
            )
            .expect("circumference stage")
            .expect("resolved circumference stage");
        let confirmed = resolved
            .confirmed
            .expect("confirmed circumference candidate");
        assert_eq!(confirmed.candidate_id, candidate.id);
        assert_eq!(confirmed.relations, candidate.relations);
        assert_eq!(confirmed.references, candidate.references);

        let effects = editor.pointer_down(
            &scene,
            pointer(82, near_target.x, near_target.y, Modifiers::default()),
        );
        let (_, plan) = construction_plan_effect(&effects);
        assert!(matches!(
            plan.proposal,
            ConstructionProposal::Circle {
                center: ConstructionPoint::New(position),
                radius,
            } if model_points_close(position, center_position)
                && (radius - expected_radius).abs() < 1.0e-12
        ));
        assert!(matches!(
            plan.relations.as_slice(),
            [InferredRelation::PointOnCurve {
                point: DraftPointSlot::Existing(point),
                contact: DraftContactDescriptor {
                    span: DraftSpanSlot::Created {
                        curve_index: 0,
                        segment: 0,
                    },
                    domain: ContactDomain::Periodic { period },
                    parameter,
                    winding: 0,
                    neighborhood: ContactNeighborhood::Interior,
                },
            }] if *point == points[1]
                && period.to_bits() == std::f64::consts::TAU.to_bits()
                && (*parameter - expected_parameter).abs() < 1.0e-12
        ));

        let mut committed = document.clone();
        let result = plan
            .apply(&mut committed)
            .expect("circle-through-point plan");
        assert_eq!(
            result.construction.points.len(),
            1,
            "only the center is new"
        );
        assert_eq!(
            result.construction.scalars.len(),
            1,
            "only the radius is new"
        );
        assert_eq!(result.construction.curves.len(), 1);
        assert_eq!(result.contacts.len(), 1);
        assert_eq!(result.constraints.len(), 1);
        let contact = committed
            .contact(result.contacts[0].contact)
            .expect("circle contact");
        assert_eq!(
            contact.curve,
            CurveSpan::line(result.construction.curves[0])
        );
        assert!(matches!(
            committed
                .constraint(result.constraints[0].constraint)
                .expect("point-on-circle constraint")
                .definition,
            DocumentConstraintDefinition::PointOnCurve { point, contact: relation_contact }
                if point == points[1] && relation_contact == result.contacts[0].contact
        ));
    }

    #[test]
    fn circle_circumference_suppression_is_raw_and_zero_radius_never_publishes() {
        let (document, _, _) = line_document();
        let scene = scene(&document);
        let center_position = [0.0, 0.0];
        let raw_radius_position = [4.05, 1.03];
        let mut suppressed = ConstraintEditor::default();
        suppressed.activate_tool(EditorTool::Circle);
        let center = scene.viewport.model_to_screen(center_position);
        suppressed.pointer_down(
            &scene,
            pointer(83, center.x, center.y, Modifiers::default()),
        );
        let radius = scene.viewport.model_to_screen(raw_radius_position);
        let effects = suppressed.pointer_down_with_draft_inference(
            &scene,
            pointer(83, radius.x, radius.y, Modifiers::default()),
            DraftInferenceInput {
                suppressed: true,
                preferred_candidate: None,
            },
        );
        assert!(effects.iter().any(|effect| matches!(
            effect,
            EditorEffect::CommitConstruction {
                proposal: ConstructionProposal::Circle { radius, .. },
                ..
            } if (*radius
                - raw_radius_position[0].hypot(raw_radius_position[1]))
                .abs()
                < 1.0e-12
        )));
        assert!(
            !effects
                .iter()
                .any(|effect| matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
        );

        let mut zero_radius = ConstraintEditor::default();
        zero_radius.activate_tool(EditorTool::Circle);
        let existing = scene.viewport.model_to_screen([-4.0, 1.0]);
        zero_radius.pointer_down(
            &scene,
            pointer(84, existing.x, existing.y, Modifiers::default()),
        );
        let effects = zero_radius.pointer_down(
            &scene,
            pointer(84, existing.x, existing.y, Modifiers::default()),
        );
        assert!(!has_construction_commit(&effects));
        assert!(zero_radius.pending_construction_commit_token().is_none());
        assert_eq!(
            zero_radius
                .draft
                .as_ref()
                .expect("valid center stage remains active")
                .points
                .len(),
            1
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the native curve-family matrix verifies one complete anchor-to-commit contract per row"
    )]
    fn point_on_curve_inference_is_exact_across_every_native_curve_family() {
        for (family, document, span, probe_parameter) in native_point_on_curve_fixtures() {
            let scene = scene(&document);
            let probe = document
                .evaluate_curve_jet(span, probe_parameter)
                .unwrap_or_else(|error| panic!("{family} probe evaluation failed: {error}"))
                .position;
            let probe_screen = scene.viewport.model_to_screen([probe.x, probe.y]);
            let anchor = inference_anchors(&scene, probe_screen)
                .into_iter()
                .find(|anchor| {
                    matches!(
                        anchor,
                        DraftReferenceAnchor::CurvePoint { contact, .. }
                            | DraftReferenceAnchor::AffineSupport { contact, .. }
                            if contact.span == span
                    )
                })
                .unwrap_or_else(|| panic!("{family} PointOnCurve anchor"));
            let (anchor_contact, anchor_position) = match anchor {
                DraftReferenceAnchor::CurvePoint {
                    contact,
                    model_position,
                    ..
                }
                | DraftReferenceAnchor::AffineSupport {
                    contact,
                    model_position,
                    ..
                } => (contact, model_position),
                DraftReferenceAnchor::PersistentPoint { .. }
                | DraftReferenceAnchor::Midpoint { .. } => unreachable!(),
            };
            let expected_domain = painted_contact_domain(&document, span)
                .unwrap_or_else(|error| panic!("{family} painted domain: {error}"));
            assert_eq!(anchor_contact.domain, expected_domain, "{family} domain");
            let anchor_total_parameter = match anchor_contact.domain {
                ContactDomain::Periodic { period } => {
                    period.mul_add(f64::from(anchor_contact.winding), anchor_contact.parameter)
                }
                ContactDomain::Bounded { .. } | ContactDomain::SupportingLine => {
                    assert_eq!(anchor_contact.winding, 0, "{family} bounded winding");
                    anchor_contact.parameter
                }
            };
            let expected_neighborhood = document
                .picked_contact_neighborhood(span, anchor_total_parameter)
                .unwrap_or_else(|error| panic!("{family} picked neighborhood: {error}"));
            assert_eq!(
                anchor_contact.neighborhood, expected_neighborhood,
                "{family} picked neighborhood"
            );
            let anchor_exact = document
                .evaluate_curve_jet(span, anchor_total_parameter)
                .unwrap_or_else(|error| panic!("{family} exact anchor evaluation: {error}"))
                .position;
            assert!(
                model_points_close(anchor_position, [anchor_exact.x, anchor_exact.y]),
                "{family} anchor position must come from exact curve evaluation"
            );

            let mut editor = ConstraintEditor::default();
            editor.activate_tool(EditorTool::Point);
            let effects = editor.pointer_down(
                &scene,
                pointer(81, probe_screen.x, probe_screen.y, Modifiers::default()),
            );
            let (_, plan) = construction_plan_effect(&effects);
            let resolution = editor
                .draft_inference_resolution()
                .unwrap_or_else(|| panic!("{family} retained resolution"));
            let DraftInferenceStatus::Resolved { candidate } = resolution.status else {
                panic!("{family} should resolve one candidate: {resolution:?}");
            };
            let candidate = resolution
                .candidates
                .iter()
                .find(|item| item.id == candidate)
                .unwrap_or_else(|| panic!("{family} resolved candidate"));
            assert!(model_points_close(
                candidate.adjusted_model_position,
                anchor_position
            ));
            assert!(matches!(
                candidate.relations.as_slice(),
                [DraftInferenceRelation::PointOnCurve { contact }]
                    if *contact == anchor_contact
            ));
            assert!(matches!(
                plan.proposal,
                ConstructionProposal::Point {
                    point: ConstructionPoint::New(position)
                } if model_points_close(position, anchor_position)
            ));
            assert!(matches!(
                plan.relations.as_slice(),
                [InferredRelation::PointOnCurve {
                    point: DraftPointSlot::Created { point_index: 0 },
                    contact: DraftContactDescriptor {
                        span: DraftSpanSlot::Existing(existing),
                        domain,
                        parameter,
                        winding,
                        neighborhood,
                    },
                }] if *existing == span
                    && *domain == anchor_contact.domain
                    && parameter.to_bits() == anchor_contact.parameter.to_bits()
                    && *winding == anchor_contact.winding
                    && *neighborhood == anchor_contact.neighborhood
            ));

            let mut committed = document.clone();
            let result = plan
                .apply(&mut committed)
                .unwrap_or_else(|error| panic!("{family} commit plan: {error}"));
            assert_eq!(result.construction.points.len(), 1, "{family} point slot");
            assert_eq!(result.contacts.len(), 1, "{family} contact slot");
            let contact = committed
                .contact(result.contacts[0].contact)
                .unwrap_or_else(|| panic!("{family} committed contact"));
            assert_eq!(contact.curve, span, "{family} existing span");
            assert_eq!(contact.domain, expected_domain, "{family} committed domain");
            assert_eq!(contact.winding, anchor_contact.winding, "{family} winding");
            assert_eq!(
                contact.neighborhood, anchor_contact.neighborhood,
                "{family} committed neighborhood"
            );
        }
    }

    #[test]
    fn curved_inference_corrects_display_chord_samples_with_exact_domain_evaluation() {
        let (_, document, span, _) = native_point_on_curve_fixtures()
            .into_iter()
            .find(|(family, ..)| *family == "cubic Bezier")
            .expect("cubic fixture");
        let base = scene(&document);
        let scene = EditorScene::from_accepted(
            base.accepted_revision,
            base.design_identity,
            &document,
            base.viewport,
            1_000.0,
        )
        .expect("coarsely tessellated scene");
        let probe = document
            .evaluate_curve_jet(span, 0.31)
            .expect("probe")
            .position;
        let pointer = scene.viewport.model_to_screen([probe.x, probe.y]);
        let anchor = inference_anchors(&scene, pointer)
            .into_iter()
            .find_map(|anchor| match anchor {
                DraftReferenceAnchor::CurvePoint {
                    contact,
                    model_position,
                    ..
                } if contact.span == span => Some((contact, model_position)),
                DraftReferenceAnchor::PersistentPoint { .. }
                | DraftReferenceAnchor::Midpoint { .. }
                | DraftReferenceAnchor::CurvePoint { .. }
                | DraftReferenceAnchor::AffineSupport { .. } => None,
            })
            .expect("curved anchor");
        let curve = scene
            .curves
            .iter()
            .find(|curve| curve.span == span)
            .expect("scene curve");
        let (segment, parameters) = curve
            .screen_polyline
            .windows(2)
            .zip(curve.screen_parameters.windows(2))
            .find(|(_, parameters)| {
                (parameters[0].min(parameters[1])..=parameters[0].max(parameters[1]))
                    .contains(&anchor.0.parameter)
            })
            .expect("owning display chord");
        let ratio = (anchor.0.parameter - parameters[0]) / (parameters[1] - parameters[0]);
        let chord_position = scene.viewport.screen_to_model(ScreenPoint {
            x: (segment[1].x - segment[0].x).mul_add(ratio, segment[0].x),
            y: (segment[1].y - segment[0].y).mul_add(ratio, segment[0].y),
        });
        let exact = document
            .evaluate_curve_jet(span, anchor.0.parameter)
            .expect("exact correction")
            .position;
        let exact_position = [exact.x, exact.y];
        assert!(model_points_close(anchor.1, exact_position));
        assert!(
            (chord_position[0] - exact_position[0]).hypot(chord_position[1] - exact_position[1])
                > 1.0e-6,
            "fixture must distinguish tessellation-chord interpolation from exact curve evaluation"
        );
    }

    #[test]
    fn standalone_point_on_existing_identity_is_history_neutral() {
        let (document, _, points) = line_document();
        let session = geosolve_sketch::RetainedSketchDocumentSession::new(
            document,
            geosolve_sketch::DocumentSolveRequest::default(),
            geosolve_sketch::SolverConfig::default(),
        )
        .expect("session");
        let accepted = session
            .accepted_state_for_current_input()
            .expect("accepted state");
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            accepted.design_identity(),
            accepted.document(),
            session.design_document(),
            Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport"),
            0.5,
        )
        .expect("scene")
        .with_retained_session(&session)
        .expect("bound scene");
        let mut coordinator =
            RetainedEditorCoordinator::new(session).expect("retained coordinator");
        coordinator.editor_mut().activate_tool(EditorTool::Point);
        let history = coordinator.history_len();
        let design = coordinator.session().design_identity();
        let endpoint = scene.viewport.model_to_screen([-4.0, 1.0]);
        let effects = coordinator.pointer_down(
            &scene,
            pointer(44, endpoint.x, endpoint.y, Modifiers::default()),
        );
        assert!(!has_construction_commit(&effects));
        assert!(
            coordinator
                .editor()
                .pending_construction_commit_token()
                .is_none()
        );
        assert_eq!(coordinator.history_len(), history);
        assert_eq!(coordinator.session().design_identity(), design);
        assert!(
            coordinator
                .session()
                .design_document()
                .point(points[0])
                .is_some()
        );
    }

    #[test]
    fn point_identity_preview_direction_and_branch_share_the_accepted_operand() {
        let (mut coordinator, scene, existing) = point_identity_branch_fixture();
        let raw_start = [0.1, 0.0];
        let start = scene.viewport.model_to_screen(raw_start);
        let first =
            coordinator.pointer_down(&scene, pointer(63, start.x, start.y, Modifiers::default()));
        assert!(!has_construction_commit(&first));
        let draft = coordinator.editor().draft.as_ref().expect("line prefix");
        assert_eq!(draft.points.len(), 1);
        assert!(matches!(
            draft.points[0],
            ConstructionPoint::Existing { id, position }
                if id == existing && model_points_close(position, [0.0, 0.0])
        ));
        assert!(model_points_close(draft.positions[0], [0.0, 0.0]));
        assert!(matches!(
            draft.confirmed_inference[0].relations.as_slice(),
            [DraftInferenceRelation::PointIdentity { point }] if *point == existing
        ));

        let raw_end = [0.15, 0.0];
        let end = scene.viewport.model_to_screen(raw_end);
        let suppressed = DraftInferenceInput {
            suppressed: true,
            preferred_candidate: None,
        };
        let preview = coordinator.editor_mut().pointer_move_with_draft_inference(
            &scene,
            pointer(63, end.x, end.y, Modifiers::default()),
            suppressed,
        );
        assert!(preview.iter().any(|effect| matches!(
            effect,
            EditorEffect::PreviewConstruction(ConstructionPreview::Complete {
                proposal: ConstructionProposal::Line {
                    start: ConstructionPoint::Existing { id, position },
                    ..
                },
                geometry: ConstructionPreviewGeometry::Polyline { points },
            }) if *id == existing
                && model_points_close(*position, [0.0, 0.0])
                && points.len() == 2
                && model_points_close(points[0], [0.0, 0.0])
                && model_points_close(points[1], raw_end)
        )));

        let effects = coordinator.pointer_down_with_draft_inference(
            &scene,
            pointer(63, end.x, end.y, Modifiers::default()),
            suppressed,
        );
        let (token, plan) = construction_plan_effect(&effects);
        assert!(matches!(
            plan.proposal,
            ConstructionProposal::Line {
                start: ConstructionPoint::Existing { id, position },
                ..
            } if id == existing && model_points_close(position, [0.0, 0.0])
        ));
        assert!(plan.relations.is_empty());
        let effect = effects
            .iter()
            .find(|effect| matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
            .expect("commit plan effect");
        let outcome = coordinator
            .apply_editor_effect(effect)
            .expect("accepted inferred construction")
            .expect("construction mutation");
        let EditorMutation::InferredConstruction(result) = outcome.value else {
            panic!("expected inferred construction mutation")
        };
        let curve = result.construction.curves[0];
        let accepted = coordinator
            .session()
            .accepted_state_for_current_input()
            .expect("accepted inferred line");
        let CurveDefinition::Line {
            start,
            end,
            branch_direction,
        } = accepted
            .document()
            .curve(curve)
            .expect("inferred line")
            .definition
        else {
            panic!("expected line definition")
        };
        let solved = accepted.document();
        let start = solved.point(start).expect("line start").position;
        let end = solved.point(end).expect("line end").position;
        assert!(model_points_close(start, [0.0, 0.0]));
        assert!(model_points_close(end, raw_end));
        assert!(branch_direction[0] > 0.0);
        assert!(branch_direction[1].abs() < 1.0e-12);
        assert!(
            coordinator
                .acknowledge_construction_commit(token, true)
                .iter()
                .any(|effect| matches!(effect, EditorEffect::ClearConstructionPreview))
        );
    }

    #[test]
    fn inference_binding_rejects_modified_same_identity_accepted_geometry() {
        let (document, _, points) = line_document();
        let session = geosolve_sketch::RetainedSketchDocumentSession::new(
            document,
            geosolve_sketch::DocumentSolveRequest::default(),
            geosolve_sketch::SolverConfig::default(),
        )
        .expect("session");
        let accepted = session
            .accepted_state_for_current_input()
            .expect("accepted state");
        let mut modified = accepted.document().clone();
        modified
            .set_point_position(points[0], [40.0, 10.0])
            .expect("same-identity modified geometry");
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            accepted.design_identity(),
            &modified,
            session.design_document(),
            Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport"),
            0.5,
        )
        .expect("structurally valid detached scene");
        assert!(matches!(
            scene.with_retained_session(&session),
            Err(EditorError::StalePreparedSketchInput)
        ));
    }

    #[test]
    fn inference_binding_rejects_mutated_public_scene_semantics() {
        let (document, _, _) = line_document();
        let session = geosolve_sketch::RetainedSketchDocumentSession::new(
            document,
            geosolve_sketch::DocumentSolveRequest::default(),
            geosolve_sketch::SolverConfig::default(),
        )
        .expect("session");
        let accepted = session
            .accepted_state_for_current_input()
            .expect("accepted state");
        let mut scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            accepted.design_identity(),
            accepted.document(),
            session.design_document(),
            Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport"),
            0.5,
        )
        .expect("scene");
        scene.curves[0].role = GeometryRole::Construction;
        assert!(matches!(
            scene.with_retained_session(&session),
            Err(EditorError::StalePreparedSketchInput)
        ));
    }

    #[test]
    fn public_scene_mutation_revokes_inferred_publication_authority() {
        let session = geosolve_sketch::RetainedSketchDocumentSession::new(
            SketchDocument::new(10.0).expect("document"),
            geosolve_sketch::DocumentSolveRequest::default(),
            geosolve_sketch::SolverConfig::default(),
        )
        .expect("session");
        let accepted = session
            .accepted_state_for_current_input()
            .expect("accepted state");
        let mut scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            accepted.design_identity(),
            accepted.document(),
            session.design_document(),
            Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport"),
            0.5,
        )
        .expect("scene")
        .with_retained_session(&session)
        .expect("bound scene");

        // These presentation fields remain public for compatibility, but a
        // caller must not retain the bound session's authority after changing
        // an inference-visible lifecycle or geometric value.
        scene.accepted_revision = scene.accepted_revision.wrapping_add(1);

        let mut coordinator =
            RetainedEditorCoordinator::new(session).expect("retained coordinator");
        coordinator.editor_mut().activate_tool(EditorTool::Line);
        let history = coordinator.history_len();
        let first = scene.viewport.model_to_screen([0.0, 0.0]);
        assert!(
            coordinator
                .pointer_down(&scene, pointer(64, first.x, first.y, Modifiers::default()))
                .iter()
                .all(|effect| !matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
        );
        let second = scene.viewport.model_to_screen([2.0, 0.01]);
        coordinator.editor_mut().pointer_move(
            &scene,
            pointer(64, second.x, second.y, Modifiers::default()),
        );
        assert!(matches!(
            coordinator
                .editor()
                .draft_inference_resolution()
                .and_then(resolved_draft_inference_candidate)
                .map(|candidate| candidate.relations.as_slice()),
            Some([DraftInferenceRelation::Horizontal])
        ));
        let effects = coordinator.pointer_down(
            &scene,
            pointer(64, second.x, second.y, Modifiers::default()),
        );
        assert!(
            effects
                .iter()
                .all(|effect| !matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
        );
        assert!(
            coordinator
                .editor()
                .pending_construction_commit_token()
                .is_none()
        );
        assert_eq!(coordinator.history_len(), history);
    }

    #[test]
    fn line_plan_rejection_restores_only_the_preterminal_prefix() {
        let document = SketchDocument::new(10.0).expect("document");
        let scene = scene(&document);
        let mut editor = ConstraintEditor::default();
        editor.activate_tool(EditorTool::Line);
        let first = scene.viewport.model_to_screen([0.0, 0.0]);
        editor.pointer_down(&scene, pointer(7, first.x, first.y, Modifiers::default()));
        let inferred = scene.viewport.model_to_screen([2.0, 0.01]);
        let effects = editor.pointer_down(
            &scene,
            pointer(7, inferred.x, inferred.y, Modifiers::default()),
        );
        let (token, plan) = construction_plan_effect(&effects);
        assert!(matches!(
            plan.relations.as_slice(),
            [InferredRelation::Horizontal {
                line: DraftSpanSlot::Created {
                    curve_index: 0,
                    segment: 0
                }
            }]
        ));
        assert!(
            editor
                .pointer_move(
                    &scene,
                    pointer(7, inferred.x + 20.0, inferred.y, Modifiers::default())
                )
                .is_empty()
        );
        assert!(editor.activate_tool(EditorTool::Point).is_empty());
        assert_eq!(editor.tool(), EditorTool::Line);
        let inference_policy = editor.draft_inference_policy();
        let invalid_inference_policy = DraftInferencePolicy {
            point_tracking: DraftInferenceBehavior {
                show_guides: true,
                adjust_coordinates: false,
                persist_constraint: true,
            },
            ..inference_policy
        };
        assert_eq!(
            editor
                .set_draft_inference_policy(invalid_inference_policy)
                .expect_err("invalid policy must reject even while publication is pending"),
            DraftInferenceError::InvalidPolicy
        );
        assert_eq!(editor.draft_inference_policy(), inference_policy);
        assert_eq!(editor.pending_construction_commit_token(), Some(token));
        let mut changed_inference_policy = inference_policy;
        changed_inference_policy.horizontal = DraftInferenceBehavior::tracking_only();
        assert!(
            editor
                .set_draft_inference_policy(changed_inference_policy)
                .expect("valid policy")
                .is_empty()
        );
        assert_eq!(editor.draft_inference_policy(), inference_policy);
        let geometry_policy = editor.geometry_interaction_policy();
        assert!(
            editor
                .set_geometry_pick_scope(GeometryPickScope::Construction)
                .is_empty()
        );
        assert_eq!(editor.geometry_interaction_policy(), geometry_policy);
        editor.set_authoring_geometry_role(GeometryRole::Construction);
        assert_eq!(editor.authoring_geometry_role(), GeometryRole::Profile);
        let stale_token = ConstructionCommitToken(token.get().wrapping_add(1));
        assert!(
            editor
                .acknowledge_construction_commit(stale_token, false)
                .is_empty()
        );
        assert_eq!(editor.pending_construction_commit_token(), Some(token));
        assert!(editor.draft_inference_resolution().is_some());
        assert!(
            editor
                .acknowledge_construction_commit(token, false)
                .is_empty()
        );

        let replacement = scene.viewport.model_to_screen([2.0, 1.0]);
        let preview = editor.pointer_move(
            &scene,
            pointer(7, replacement.x, replacement.y, Modifiers::default()),
        );
        assert!(preview.iter().any(|effect| matches!(
            effect,
            EditorEffect::PreviewConstruction(ConstructionPreview::Complete {
                proposal: ConstructionProposal::Line {
                    start: ConstructionPoint::New(start),
                    end: ConstructionPoint::New(end),
                },
                ..
            }) if model_points_close(*start, [0.0, 0.0])
                && model_points_close(*end, [2.0, 1.0])
        )));
    }

    #[test]
    fn coordinator_plan_success_waits_for_ack_before_clearing_publication() {
        let session = geosolve_sketch::RetainedSketchDocumentSession::new(
            SketchDocument::new(10.0).expect("document"),
            geosolve_sketch::DocumentSolveRequest::default(),
            geosolve_sketch::SolverConfig::default(),
        )
        .expect("session");
        let accepted = session
            .accepted_state_for_current_input()
            .expect("accepted state");
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            accepted.design_identity(),
            accepted.document(),
            session.design_document(),
            Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport"),
            0.5,
        )
        .expect("scene")
        .with_retained_session(&session)
        .expect("bound scene");
        let mut coordinator =
            RetainedEditorCoordinator::new(session).expect("retained coordinator");
        coordinator.editor_mut().activate_tool(EditorTool::Line);
        let first = scene.viewport.model_to_screen([0.0, 0.0]);
        coordinator.pointer_down(&scene, pointer(45, first.x, first.y, Modifiers::default()));
        let second = scene.viewport.model_to_screen([2.0, 0.01]);
        let effects = coordinator.pointer_down(
            &scene,
            pointer(45, second.x, second.y, Modifiers::default()),
        );
        let (token, _) = construction_plan_effect(&effects);
        let commit = effects
            .iter()
            .find(|effect| matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
            .expect("commit effect");
        let history = coordinator.history_len();
        let outcome = coordinator
            .apply_editor_effect(commit)
            .expect("plan publication")
            .expect("mutation");
        assert!(matches!(
            outcome.value,
            EditorMutation::InferredConstruction(_)
        ));
        assert_eq!(coordinator.history_len(), history + 1);
        assert!(coordinator.editor().draft_inference_resolution().is_some());
        let stale_token = ConstructionCommitToken(token.get().wrapping_add(1));
        assert!(
            coordinator
                .acknowledge_construction_commit(stale_token, true)
                .is_empty()
        );
        assert_eq!(
            coordinator.editor().pending_construction_commit_token(),
            Some(token)
        );
        assert!(coordinator.editor().draft_inference_resolution().is_some());
        let acknowledged = coordinator.acknowledge_construction_commit(token, true);
        assert!(
            acknowledged
                .iter()
                .any(|effect| matches!(effect, EditorEffect::ClearConstructionPreview))
        );
        assert!(
            acknowledged
                .iter()
                .any(|effect| matches!(effect, EditorEffect::DraftInferenceChanged(None)))
        );
        assert!(coordinator.editor().draft_inference_resolution().is_none());
        assert!(
            coordinator
                .editor()
                .pending_construction_commit_token()
                .is_none()
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the real scene-to-coordinator checkpoint keeps every wake, display, and atomic-publication assertion in one ordered transition"
    )]
    fn native_midpoint_normal_is_one_exact_scene_editor_coordinator_checkpoint() {
        let (document, lines, _) = line_document();
        let session = geosolve_sketch::RetainedSketchDocumentSession::new(
            document,
            geosolve_sketch::DocumentSolveRequest::default(),
            geosolve_sketch::SolverConfig::default(),
        )
        .expect("session");
        let accepted = session
            .accepted_state_for_current_input()
            .expect("accepted state");
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            accepted.design_identity(),
            accepted.document(),
            session.design_document(),
            Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport"),
            0.5,
        )
        .expect("scene")
        .with_retained_session(&session)
        .expect("bound scene");
        let mut coordinator =
            RetainedEditorCoordinator::new(session).expect("retained coordinator");
        let _ = coordinator
            .editor_mut()
            .set_geometry_visibility(native_geometry_policy().visibility);
        coordinator.editor_mut().activate_tool(EditorTool::Line);
        let history = coordinator.history_len();

        let midpoint = scene.viewport.model_to_screen([0.0, 1.0]);
        coordinator.editor_mut().pointer_move(
            &scene,
            pointer(46, midpoint.x, midpoint.y, Modifiers::default()),
        );
        let awakened = coordinator
            .editor()
            .draft_inference_resolution()
            .and_then(resolved_draft_inference_candidate)
            .expect("awakened midpoint candidate");
        assert!(matches!(
            awakened.relations.as_slice(),
            [DraftInferenceRelation::Midpoint { span }] if *span == lines[0]
        ));
        assert!(
            coordinator
                .editor()
                .draft_inference_engine
                .remembered_references()
                .iter()
                .any(|reference| matches!(
                    reference,
                    DraftReferenceAnchor::Midpoint { span, .. } if *span == lines[0]
                ))
        );

        let left_anchor = scene.viewport.model_to_screen([1.0, 3.0]);
        coordinator.editor_mut().pointer_move(
            &scene,
            pointer(46, left_anchor.x, left_anchor.y, Modifiers::default()),
        );
        assert!(coordinator.editor().draft_inference_resolution().is_none());
        assert!(
            coordinator
                .editor()
                .draft_inference_engine
                .remembered_references()
                .iter()
                .any(|reference| matches!(
                    reference,
                    DraftReferenceAnchor::Midpoint { span, .. } if *span == lines[0]
                ))
        );

        let first = coordinator.pointer_down(
            &scene,
            pointer(46, midpoint.x, midpoint.y, Modifiers::default()),
        );
        assert!(!has_construction_commit(&first));
        assert_eq!(
            coordinator
                .editor()
                .draft
                .as_ref()
                .expect("line prefix")
                .positions,
            vec![[0.0, 1.0]]
        );
        assert!(matches!(
            coordinator
                .editor()
                .draft_inference_engine
                .remembered_references(),
            [DraftReferenceAnchor::Midpoint { span, .. }] if *span == lines[0]
        ));

        let near_normal = scene.viewport.model_to_screen([0.05, 4.0]);
        coordinator.editor_mut().pointer_move(
            &scene,
            pointer(46, near_normal.x, near_normal.y, Modifiers::default()),
        );
        let displayed = coordinator
            .editor()
            .draft_inference_resolution()
            .and_then(resolved_draft_inference_candidate)
            .expect("displayed midpoint-normal candidate");
        assert!(matches!(
            displayed.relations.as_slice(),
            [DraftInferenceRelation::Perpendicular { reference }]
                if *reference == lines[0]
        ));
        let effects = coordinator.pointer_down(
            &scene,
            pointer(46, near_normal.x, near_normal.y, Modifiers::default()),
        );
        let (token, plan) = construction_plan_effect(&effects);
        assert!(matches!(
            plan.proposal,
            ConstructionProposal::Line {
                start: ConstructionPoint::New(start),
                end: ConstructionPoint::New(end),
            } if model_points_close(start, [0.0, 1.0])
                && model_points_close(end, [0.0, 4.0])
        ));
        assert!(
            matches!(
                plan.relations.as_slice(),
                [
                    InferredRelation::Midpoint {
                        point: DraftPointSlot::Created { point_index: 0 },
                        line: DraftSpanSlot::Existing(midpoint_line),
                    },
                    InferredRelation::Perpendicular {
                        first: DraftSpanSlot::Created {
                            curve_index: 0,
                            segment: 0,
                        },
                        second: DraftSpanSlot::Existing(normal_line),
                    },
                ] if *midpoint_line == lines[0] && *normal_line == lines[0]
            ),
            "unexpected midpoint-normal plan: {:?}",
            plan.relations
        );

        let commit = effects
            .iter()
            .find(|effect| matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
            .expect("commit effect");
        let outcome = coordinator
            .apply_editor_effect(commit)
            .expect("plan publication")
            .expect("retained mutation");
        let EditorMutation::InferredConstruction(result) = outcome.value else {
            panic!("expected inferred construction mutation");
        };
        assert_eq!(result.constraints.len(), 2);
        assert_eq!(coordinator.history_len(), history + 1);
        let accepted = coordinator
            .session()
            .accepted_state_for_current_input()
            .expect("accepted atomic midpoint-normal publication");
        assert_eq!(
            accepted.design_identity(),
            coordinator.session().design_identity()
        );
        assert!(matches!(
            coordinator
                .session()
                .design_document()
                .constraint(result.constraints[0].constraint)
                .expect("midpoint constraint")
                .definition,
            DocumentConstraintDefinition::Midpoint { line, .. } if line == lines[0]
        ));
        assert!(matches!(
            coordinator
                .session()
                .design_document()
                .constraint(result.constraints[1].constraint)
                .expect("perpendicular constraint")
                .definition,
            DocumentConstraintDefinition::Perpendicular { second, .. } if second == lines[0]
        ));
        let acknowledged = coordinator.acknowledge_construction_commit(token, true);
        assert!(
            acknowledged
                .iter()
                .any(|effect| matches!(effect, EditorEffect::ClearConstructionPreview))
        );
    }

    fn pending_horizontal_plan_fixture() -> (RetainedEditorCoordinator, EditorEffect) {
        let mut document = SketchDocument::new(10.0).expect("document");
        document
            .add_external_binding(
                "unused external point",
                geosolve_sketch::ExternalFeatureKindV1::Point,
                None,
            )
            .expect("external binding");
        let session = geosolve_sketch::RetainedSketchDocumentSession::new(
            document,
            geosolve_sketch::DocumentSolveRequest::default(),
            geosolve_sketch::SolverConfig::default(),
        )
        .expect("session");
        let accepted = session
            .accepted_state_for_current_input()
            .expect("accepted state");
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            accepted.design_identity(),
            accepted.document(),
            session.design_document(),
            Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport"),
            0.5,
        )
        .expect("scene")
        .with_retained_session(&session)
        .expect("bound scene");
        let mut coordinator =
            RetainedEditorCoordinator::new(session).expect("retained coordinator");
        coordinator.editor_mut().activate_tool(EditorTool::Line);
        for position in [[0.0, 0.0], [2.0, 0.01]] {
            let screen = scene.viewport.model_to_screen(position);
            let effects = coordinator.pointer_down(
                &scene,
                pointer(73, screen.x, screen.y, Modifiers::default()),
            );
            if let Some(effect) = effects
                .into_iter()
                .find(|effect| matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
            {
                return (coordinator, effect);
            }
        }
        panic!("pending construction-plan effect was not emitted");
    }

    fn construction_auth_state(
        coordinator: &RetainedEditorCoordinator,
    ) -> (
        geosolve_sketch::SketchDesignIdentity,
        geosolve_sketch::SketchAttemptIdentity,
        Option<PreparedSketchInput>,
        usize,
        usize,
        usize,
    ) {
        (
            coordinator.session().design_identity(),
            coordinator.session().last_attempt().identity(),
            coordinator.session().accepted_prepared_input(),
            coordinator.history_len(),
            coordinator.history_cursor(),
            coordinator.transcript().len(),
        )
    }

    fn assert_input_change_invalidated_pending_plan(
        coordinator: &mut RetainedEditorCoordinator,
        effect: &EditorEffect,
    ) {
        let (expected, plan) = match effect {
            EditorEffect::CommitConstructionPlan { expected, plan, .. } => (**expected, plan),
            _ => unreachable!("fixture returns a construction plan"),
        };
        assert!(
            coordinator
                .editor()
                .pending_construction_commit_token()
                .is_none()
        );
        assert!(coordinator.editor().draft_inference_resolution().is_none());
        let before = construction_auth_state(coordinator);
        assert!(matches!(
            coordinator.apply_editor_effect(effect),
            Err(CoordinatorError::InferredConstructionCommitMismatch)
        ));
        assert!(matches!(
            coordinator.apply_construction_plan(&expected, plan),
            Err(CoordinatorError::StaleInferredConstructionInput)
        ));
        assert_eq!(construction_auth_state(coordinator), before);
    }

    #[test]
    fn pending_plan_authentication_rejects_token_and_plan_substitution_then_accepts_original() {
        let (mut coordinator, original) = pending_horizontal_plan_fixture();
        let (token, original_plan) = match &original {
            EditorEffect::CommitConstructionPlan { token, plan, .. } => (*token, plan.clone()),
            _ => unreachable!("fixture returns a construction plan"),
        };
        let before = construction_auth_state(&coordinator);

        let mut wrong_token = original.clone();
        let EditorEffect::CommitConstructionPlan {
            token: candidate, ..
        } = &mut wrong_token
        else {
            unreachable!("cloned construction plan")
        };
        *candidate = ConstructionCommitToken(token.get().wrapping_add(1));
        assert!(matches!(
            coordinator.apply_editor_effect(&wrong_token),
            Err(CoordinatorError::InferredConstructionCommitMismatch)
        ));
        assert_eq!(construction_auth_state(&coordinator), before);
        assert_eq!(
            coordinator.editor().pending_construction_commit_token(),
            Some(token)
        );

        let mut substituted = original.clone();
        let EditorEffect::CommitConstructionPlan { plan, .. } = &mut substituted else {
            unreachable!("cloned construction plan")
        };
        plan.role = match plan.role {
            GeometryRole::Profile => GeometryRole::Construction,
            GeometryRole::Construction => GeometryRole::Profile,
        };
        assert_ne!(*plan, original_plan);
        assert!(matches!(
            coordinator.apply_editor_effect(&substituted),
            Err(CoordinatorError::InferredConstructionCommitMismatch)
        ));
        let EditorEffect::CommitConstructionPlan { expected, plan, .. } = &substituted else {
            unreachable!("cloned construction plan")
        };
        assert!(matches!(
            coordinator.apply_construction_plan(expected.as_ref(), plan),
            Err(CoordinatorError::InferredConstructionCommitMismatch)
        ));
        assert_eq!(construction_auth_state(&coordinator), before);
        assert_eq!(
            coordinator.editor().pending_construction_commit_token(),
            Some(token)
        );

        assert!(matches!(
            coordinator.apply_editor_effect(&original),
            Ok(Some(MutationOutcome {
                value: EditorMutation::InferredConstruction(_),
                ..
            }))
        ));
        assert_eq!(
            coordinator.editor().pending_construction_commit_token(),
            Some(token)
        );
    }

    #[test]
    fn construction_plan_effect_without_its_pending_token_is_state_neutral() {
        let (mut coordinator, effect) = pending_horizontal_plan_fixture();
        let token = match &effect {
            EditorEffect::CommitConstructionPlan { token, .. } => *token,
            _ => unreachable!("fixture returns a construction plan"),
        };
        assert!(
            coordinator
                .acknowledge_construction_commit(token, false)
                .is_empty()
        );
        assert!(
            coordinator
                .editor()
                .pending_construction_commit_token()
                .is_none()
        );
        let before = construction_auth_state(&coordinator);
        assert!(matches!(
            coordinator.apply_editor_effect(&effect),
            Err(CoordinatorError::InferredConstructionCommitMismatch)
        ));
        assert_eq!(construction_auth_state(&coordinator), before);
    }

    #[test]
    fn parameter_change_invalidates_pending_plan_exact_input_before_mutation() {
        let (mut coordinator, effect) = pending_horizontal_plan_fixture();
        let design = coordinator.session().design_identity();
        coordinator
            .replace_parameter_batch(
                design,
                geosolve_sketch::ParameterBatch::new(1, Vec::new()).expect("parameter batch"),
                geosolve_sketch::DocumentSolveRequest::default(),
            )
            .expect("parameter replacement");
        assert_input_change_invalidated_pending_plan(&mut coordinator, &effect);
    }

    #[test]
    fn external_snapshot_change_invalidates_pending_plan_exact_input_before_mutation() {
        let (mut coordinator, effect) = pending_horizontal_plan_fixture();
        let design = coordinator.session().design_identity();
        let binding = coordinator.session().design_document().external_bindings()[0].id;
        let snapshot = geosolve_sketch::ExternalSnapshotEntry {
            binding,
            source_revision: 1,
            source_digest: geosolve_sketch::ExternalSnapshotDigest::from_bytes([17; 32]),
            feature: geosolve_sketch::ExternalSnapshotFeatureV1::Point {
                position: [3.0, 4.0],
                scale: 1.0,
                resources: geosolve_sketch::ExternalSnapshotResourcesV1 {
                    point_count: 1,
                    control_count: 0,
                    span_count: 0,
                },
            },
        };
        coordinator
            .replace_external_snapshot_set(
                design,
                geosolve_sketch::ExternalSnapshotSet::new(1, vec![snapshot])
                    .expect("external snapshots"),
                geosolve_sketch::DocumentSolveRequest::default(),
            )
            .expect("external snapshot replacement");
        assert_input_change_invalidated_pending_plan(&mut coordinator, &effect);
    }

    #[test]
    fn reattempt_invalidates_pending_plan_exact_input_before_mutation() {
        let (mut coordinator, effect) = pending_horizontal_plan_fixture();
        let design = coordinator.session().design_identity();
        coordinator.reattempt(design).expect("reattempt");
        assert_input_change_invalidated_pending_plan(&mut coordinator, &effect);
    }

    #[test]
    fn polyline_direction_inference_targets_each_created_segment() {
        let document = SketchDocument::new(10.0).expect("document");
        let scene = scene(&document);
        let mut editor = ConstraintEditor::default();
        editor.activate_tool(EditorTool::Polyline);
        for position in [[0.0, 0.0], [2.0, 0.01], [2.01, 2.0]] {
            let screen = scene.viewport.model_to_screen(position);
            editor.pointer_down(&scene, pointer(9, screen.x, screen.y, Modifiers::default()));
        }
        let effects = editor.complete_draft(scene.design_identity);
        let (_, plan) = construction_plan_effect(&effects);
        assert!(matches!(
            plan.relations.as_slice(),
            [
                InferredRelation::Horizontal {
                    line: DraftSpanSlot::Created {
                        curve_index: 0,
                        segment: 0
                    }
                },
                InferredRelation::Vertical {
                    line: DraftSpanSlot::Created {
                        curve_index: 0,
                        segment: 1
                    }
                }
            ]
        ));
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "candidate guides, confirmation and final lowering are one provenance trace"
    )]
    fn compound_candidate_guides_confirmation_and_commit_plan_keep_one_identity() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let horizontal_reference = document
            .add_point("horizontal reference", [-4.0, 4.0])
            .expect("point");
        let vertical_reference = document
            .add_point("vertical reference", [3.0, -4.0])
            .expect("point");
        let scene = scene(&document);
        let pointer_id = 91;
        let mut editor = ConstraintEditor::default();
        editor.activate_tool(EditorTool::Polyline);
        let first = scene.viewport.model_to_screen([0.0, 0.0]);
        editor.pointer_down(
            &scene,
            pointer(pointer_id, first.x, first.y, Modifiers::default()),
        );

        for position in [[-4.0, 4.0], [3.0, -4.0]] {
            let screen = scene.viewport.model_to_screen(position);
            editor.pointer_move(
                &scene,
                pointer(pointer_id, screen.x, screen.y, Modifiers::default()),
            );
        }
        let raw = scene.viewport.model_to_screen([3.04, 4.05]);
        editor.pointer_move(
            &scene,
            pointer(pointer_id, raw.x, raw.y, Modifiers::default()),
        );
        let resolution = editor
            .draft_inference_resolution()
            .expect("compound resolution")
            .clone();
        let DraftInferenceStatus::Resolved {
            candidate: candidate_id,
        } = resolution.status
        else {
            panic!("resolved compound candidate");
        };
        let candidate = resolution
            .candidates
            .iter()
            .find(|candidate| candidate.id == candidate_id)
            .expect("selected candidate")
            .clone();
        assert_eq!(
            candidate.relations,
            vec![
                DraftInferenceRelation::HorizontalPoints {
                    reference: horizontal_reference,
                },
                DraftInferenceRelation::VerticalPoints {
                    reference: vertical_reference,
                },
            ]
        );
        assert!(candidate.guides.iter().enumerate().all(|(ordinal, guide)| {
            guide.id
                == DraftGuideId {
                    candidate: Some(candidate_id),
                    ordinal: u32::try_from(ordinal).expect("bounded guide ordinal"),
                }
        }));
        assert_eq!(
            resolution
                .guides
                .iter()
                .filter(|guide| guide.id.candidate.is_some())
                .copied()
                .collect::<Vec<_>>(),
            candidate.guides
        );
        let mut malformed = candidate.clone();
        malformed.guides[0].id.candidate = None;
        assert!(matches!(
            confirmed_draft_inference(&resolution, malformed, 1),
            Err(DraftInferenceError::InvalidFrame)
        ));
        let mut changed_relations = candidate.clone();
        changed_relations.relations.remove(0);
        assert!(matches!(
            confirmed_draft_inference(&resolution, changed_relations, 1),
            Err(DraftInferenceError::InvalidFrame)
        ));
        assert!(!candidate.references.is_empty());
        let mut changed_references = candidate.clone();
        changed_references.references.pop();
        assert!(matches!(
            confirmed_draft_inference(&resolution, changed_references, 1),
            Err(DraftInferenceError::InvalidFrame)
        ));

        editor.pointer_down(
            &scene,
            pointer(pointer_id, raw.x, raw.y, Modifiers::default()),
        );
        let confirmed = editor
            .draft
            .as_ref()
            .expect("polyline draft")
            .confirmed_inference
            .last()
            .expect("confirmed compound inference");
        assert_eq!(confirmed.candidate_id, candidate_id);
        assert_eq!(confirmed.relations, candidate.relations);
        assert_eq!(confirmed.references, candidate.references);

        let effects = editor.complete_draft(scene.design_identity);
        let (_, plan) = construction_plan_effect(&effects);
        assert!(matches!(
            plan.relations.as_slice(),
            [
                InferredRelation::HorizontalPoints {
                    first: DraftPointSlot::Created { point_index: 1 },
                    second: DraftPointSlot::Existing(horizontal),
                },
                InferredRelation::VerticalPoints {
                    first: DraftPointSlot::Created { point_index: 1 },
                    second: DraftPointSlot::Existing(vertical),
                },
            ] if *horizontal == horizontal_reference && *vertical == vertical_reference
        ));
    }

    #[test]
    fn direction_only_reference_does_not_leak_across_polyline_stages() {
        let (document, lines, _) = line_document();
        let scene = scene(&document);
        let mut editor = ConstraintEditor::default();
        editor.activate_tool(EditorTool::Polyline);
        let first = scene.viewport.model_to_screen([0.0, 3.0]);
        editor.pointer_down(&scene, pointer(10, first.x, first.y, Modifiers::default()));
        let line_anchor = inference_anchors(&scene, scene.viewport.model_to_screen([0.0, 1.0]))
            .into_iter()
            .find(|anchor| {
                matches!(
                    anchor,
                    DraftReferenceAnchor::AffineSupport { contact, .. }
                        if contact.span == lines[0]
                )
            })
            .expect("affine reference");
        editor
            .draft_inference_engine
            .remember_reference(line_anchor)
            .expect("remember reference");
        let second = scene.viewport.model_to_screen([2.0, 3.0]);
        editor.pointer_down(
            &scene,
            pointer(10, second.x, second.y, Modifiers::default()),
        );
        assert!(
            editor
                .draft
                .as_ref()
                .expect("polyline draft")
                .confirmed_inference
                .last()
                .is_some_and(
                    |confirmed| confirmed.relations.iter().any(|relation| matches!(
                        relation,
                        DraftInferenceRelation::Parallel { reference } if *reference == lines[0]
                    ))
                )
        );
        assert!(
            editor
                .draft_inference_engine
                .remembered_references()
                .is_empty()
        );
    }

    #[test]
    fn semantic_suppression_is_explicit_and_commits_raw_geometry() {
        let document = SketchDocument::new(10.0).expect("document");
        let scene = scene(&document);
        let mut editor = ConstraintEditor::default();
        editor.activate_tool(EditorTool::Line);
        let first = scene.viewport.model_to_screen([0.0, 0.0]);
        editor.pointer_down(&scene, pointer(11, first.x, first.y, Modifiers::default()));
        let end = scene.viewport.model_to_screen([2.0, 0.01]);
        let inference = DraftInferenceInput {
            suppressed: true,
            preferred_candidate: None,
        };
        let preview = editor.pointer_move_with_draft_inference(
            &scene,
            pointer(11, end.x, end.y, Modifiers::default()),
            inference,
        );
        assert!(preview.iter().any(|effect| matches!(
            effect,
            EditorEffect::DraftInferenceChanged(Some(DraftInferenceResolution {
                status: DraftInferenceStatus::Suppressed,
                ..
            }))
        )));
        let commit = editor.pointer_down_with_draft_inference(
            &scene,
            pointer(11, end.x, end.y, Modifiers::default()),
            inference,
        );
        assert!(commit.iter().any(|effect| matches!(
            effect,
            EditorEffect::CommitConstruction {
                proposal: ConstructionProposal::Line {
                    end: ConstructionPoint::New(position),
                    ..
                },
                ..
            } if model_points_close(*position, [2.0, 0.01])
        )));
        assert!(
            !commit
                .iter()
                .any(|effect| matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
        );
    }

    #[test]
    fn invalid_terminal_inference_frame_never_falls_through_to_raw_construction() {
        let (document, _, _) = line_document();
        let scene = scene(&document);
        let mut editor = ConstraintEditor::default();
        editor.activate_tool(EditorTool::Line);
        let start = scene.viewport.model_to_screen([0.0, 3.0]);
        editor.pointer_down(&scene, pointer(71, start.x, start.y, Modifiers::default()));
        let end = scene.viewport.model_to_screen([2.0, 3.01]);
        editor.pointer_move(&scene, pointer(71, end.x, end.y, Modifiers::default()));
        assert!(editor.draft_inference_resolution().is_some());

        let mut invalid_scene = scene.clone();
        invalid_scene.construction_snap_points[0].model_position = [f64::NAN, 0.0];
        let effects = editor.pointer_down(
            &invalid_scene,
            pointer(71, end.x, end.y, Modifiers::default()),
        );
        assert!(effects.iter().all(|effect| !matches!(
            effect,
            EditorEffect::CommitConstruction { .. } | EditorEffect::CommitConstructionPlan { .. }
        )));
        assert_eq!(
            effects,
            vec![EditorEffect::DraftInferenceChanged(None)],
            "a malformed terminal frame clears the stale guide instead of committing raw geometry"
        );
        assert!(editor.pending_construction_commit_token().is_none());
        assert_eq!(
            editor.draft.as_ref().map(|draft| draft.points.len()),
            Some(1),
            "the valid pre-terminal prefix remains correction-ready"
        );

        let retry = editor.pointer_down(&scene, pointer(71, end.x, end.y, Modifiers::default()));
        assert!(
            retry
                .iter()
                .any(|effect| matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
        );
    }

    #[test]
    fn camera_invalidation_immediately_clears_published_guides() {
        let document = SketchDocument::new(10.0).expect("document");
        let scene = scene(&document);
        let mut editor = ConstraintEditor::default();
        editor.activate_tool(EditorTool::Line);
        let first = scene.viewport.model_to_screen([0.0, 0.0]);
        editor.pointer_down(&scene, pointer(12, first.x, first.y, Modifiers::default()));
        let end = scene.viewport.model_to_screen([2.0, 0.01]);
        editor.pointer_move(&scene, pointer(12, end.x, end.y, Modifiers::default()));
        assert!(editor.draft_inference_resolution().is_some());
        assert_eq!(
            editor.invalidate_draft_inference(),
            vec![EditorEffect::DraftInferenceChanged(None)]
        );
        assert!(editor.draft_inference_resolution().is_none());
        assert!(
            editor
                .draft_inference_engine
                .remembered_references()
                .is_empty()
        );
    }

    #[test]
    fn curved_scene_tessellation_has_a_dense_baseline_without_subdividing_lines() {
        let (mut document, lines, _) = line_document();
        let controls = [[-3.0, 0.0], [-3.0, 3.0], [3.0, -3.0], [3.0, 0.0]].map(|position| {
            document
                .add_point("Bezier control", position)
                .expect("point")
        });
        let bezier = document
            .add_curve(
                "inflected Bezier",
                CurveDefinition::CubicBezier { controls },
            )
            .expect("Bezier");

        let scene = scene(&document);
        let line = scene
            .curves
            .iter()
            .find(|curve| curve.span == lines[0])
            .expect("line scene curve");
        assert_eq!(line.screen_polyline.len(), 2);

        let bezier = scene
            .curves
            .iter()
            .find(|curve| curve.span == CurveSpan::line(bezier))
            .expect("Bezier scene curve");
        let quarter = document
            .evaluate_curve_jet(bezier.span, 0.25)
            .expect("analytic Bezier quarter point");
        let quarter = scene
            .viewport
            .model_to_screen([quarter.position.x, quarter.position.y]);
        assert!(
            curve_hit(bezier, quarter, 0.5).is_some(),
            "an inflected curve whose parameter midpoint lies on its chord must remain pickable at its analytic quarter point"
        );
        assert_eq!(bezier.screen_polyline.len(), bezier.screen_parameters.len());
    }

    #[test]
    fn computed_fillet_arcs_keep_a_smooth_minimum_at_loose_tolerance() {
        let (_document, lines, _) = line_document();
        let source = geosolve_sketch_features::NativeCurveSpanSource { span: lines[0] };
        let contact = geosolve_sketch_features::ComputedFilletContact {
            source,
            parameter: 0.5,
            winding: 0,
            total_parameter: 0.5,
            position: [1.0, 0.0],
        };
        let arc = geosolve_sketch_features::ComputedCircularArc {
            center: [0.0, 0.0],
            radius: 1.0,
            start_angle: 0.0,
            end_angle: std::f64::consts::FRAC_PI_2,
            sweep: DocumentArcSweep::CounterClockwise,
            contacts: [contact, contact],
        };
        let viewport = Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport");
        let points = tessellate_computed_arc(&arc, viewport, 1.0e6).expect("computed arc");

        assert_eq!(points.len(), usize::from(MIN_COMPUTED_ARC_SEGMENTS) + 1);
        let half_step_angle =
            std::f64::consts::FRAC_PI_2 / (2.0 * f64::from(MIN_COMPUTED_ARC_SEGMENTS));
        let analytic_between_vertices =
            viewport.model_to_screen([half_step_angle.cos(), half_step_angle.sin()]);
        let distance = points
            .windows(2)
            .map(|segment| {
                point_segment_projection(analytic_between_vertices, segment[0], segment[1]).0
            })
            .min_by(f64::total_cmp)
            .expect("computed arc segments");
        assert!(distance <= 0.25, "computed arc chord error was {distance}");
    }

    struct FilletInteractionFixture {
        scene: EditorScene,
        owner: ComputedCornerRef,
        input: ComputedFeatureEvaluationInput,
        sources: [NativeCurveSpanSource; 2],
    }

    #[allow(clippy::too_many_lines)]
    fn fillet_interaction_fixture(
        pixels_per_model_unit: f64,
        derivative: [f64; 2],
    ) -> FilletInteractionFixture {
        let mut document = SketchDocument::new(10.0).expect("document");
        let first_start = document
            .add_point("first start", [2.0, -4.0])
            .expect("point");
        let first_end = document.add_point("first end", [2.0, 4.0]).expect("point");
        let second_start = document
            .add_point("second start", [-4.0, 2.0])
            .expect("point");
        let second_end = document.add_point("second end", [4.0, 2.0]).expect("point");
        let first = document
            .add_curve(
                "first parent",
                CurveDefinition::Line {
                    start: first_start,
                    end: first_end,
                    branch_direction: [0.0, 1.0],
                },
            )
            .expect("curve");
        let second = document
            .add_curve(
                "second parent",
                CurveDefinition::Line {
                    start: second_start,
                    end: second_end,
                    branch_direction: [1.0, 0.0],
                },
            )
            .expect("curve");
        let sources = [
            NativeCurveSpanSource {
                span: CurveSpan::line(first),
            },
            NativeCurveSpanSource {
                span: CurveSpan::line(second),
            },
        ];
        let session = geosolve_sketch::RetainedSketchDocumentSession::new(
            document,
            geosolve_sketch::DocumentSolveRequest::default(),
            geosolve_sketch::SolverConfig::default(),
        )
        .expect("accepted session");
        let accepted = session
            .accepted_state_for_current_input()
            .expect("accepted state");
        let features = ComputedFeatureDocument::new(session.design_document().id());
        let snapshot = geosolve_sketch_features::ComputedFeatureEvaluationSnapshot::capture(
            &session,
            &features,
            ComputedFeatureEvaluationPolicy::default(),
        )
        .expect("feature snapshot");
        let input = snapshot.input();
        let viewport =
            Viewport::new([1000.0, 700.0], [0.0, 0.0], pixels_per_model_unit).expect("viewport");
        let mut scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            accepted.design_identity(),
            accepted.document(),
            session.design_document(),
            viewport,
            0.25,
        )
        .expect("scene");
        let owner = ComputedCornerRef {
            feature: ComputedFeatureId::from_raw(7),
            corner: ComputedFeatureCornerId::from_raw(11),
        };
        let contacts = [
            ComputedFilletContact {
                source: sources[0],
                parameter: 0.5,
                winding: 0,
                total_parameter: 0.5,
                position: [2.0, 0.0],
            },
            ComputedFilletContact {
                source: sources[1],
                parameter: 0.5,
                winding: 0,
                total_parameter: 0.5,
                position: [0.0, 2.0],
            },
        ];
        let arc = ComputedCircularArc {
            center: [0.0, 0.0],
            radius: 2.0,
            start_angle: 0.0,
            end_angle: std::f64::consts::FRAC_PI_2,
            sweep: DocumentArcSweep::CounterClockwise,
            contacts,
        };
        scene.computed_curves.push(SceneComputedCurve {
            edge: ComputedEdgeId {
                evaluation: ComputedEvaluationRevision::from_raw(1),
                ordinal: 0,
            },
            owner,
            role: GeometryRole::Profile,
            center: arc.center,
            radius: arc.radius,
            start_angle: arc.start_angle,
            end_angle: arc.end_angle,
            sweep: arc.sweep,
            contacts,
            screen_polyline: tessellate_computed_arc(&arc, viewport, 0.25).expect("arc"),
            radius_rail: None,
        });
        scene.feature_identity = Some(input.features);
        scene.computed_input = Some(input);
        scene
            .attach_computed_fillet_radius_rail(owner, derivative, vec![owner])
            .expect("radius rail");
        FilletInteractionFixture {
            scene,
            owner,
            input,
            sources,
        }
    }

    fn preview_radius(effects: &[EditorEffect]) -> f64 {
        let [EditorEffect::PreviewComputedFeatureRadius { radius, .. }] = effects else {
            panic!("one radius preview expected, got {effects:?}");
        };
        *radius
    }

    fn preview_contact_parameter(effects: &[EditorEffect]) -> f64 {
        let [EditorEffect::PreviewComputedFeatureContact { parameter, .. }] = effects else {
            panic!("one contact preview expected, got {effects:?}");
        };
        *parameter
    }

    fn rebuilt_fillet_candidate_scene(fixture: &FilletInteractionFixture) -> EditorScene {
        let mut scene = fixture.scene.clone();
        let mut candidate = fixture.input;
        candidate.features.revision = ComputedFeatureRevision::from_raw(
            candidate
                .features
                .revision
                .raw()
                .checked_add(1)
                .expect("feature revision"),
        );
        scene.feature_identity = Some(candidate.features);
        scene.computed_input = Some(candidate);
        scene
    }

    fn install_test_fillet_actions(
        fixture: &mut FilletInteractionFixture,
    ) -> (SceneFilletActionTarget, SceneFilletActionTarget) {
        let viewport = fixture.scene.viewport;
        let control_start = [2.0, 0.0];
        let control_end = [2.0, -0.75];
        let alternative_model_polyline = vec![[-3.0, -3.0], [-2.5, -3.25], [-2.0, -3.0]];
        fixture
            .scene
            .set_fillet_corner_actions(
                fixture.owner,
                vec![
                    SceneFilletAction {
                        id: SceneFilletActionId::ReverseFirstRetainedDirection,
                        owner: fixture.owner,
                        label: "Reverse first retained direction".into(),
                        availability: SceneFilletActionAvailability::Applicable,
                        control_geometry: Some(SceneFilletActionControlGeometry {
                            model_anchor: control_start,
                            model_direction: [0.0, -1.0],
                            screen_start: viewport.model_to_screen(control_start),
                            screen_end: viewport.model_to_screen(control_end),
                        }),
                        dashed_alternative_arc: None,
                    },
                    SceneFilletAction {
                        id: SceneFilletActionId::LocalAlternative {
                            first: DocumentCurveNormalSide::Left,
                            second: DocumentCurveNormalSide::Right,
                        },
                        owner: fixture.owner,
                        label: "Use local side branch 1".into(),
                        availability: SceneFilletActionAvailability::Applicable,
                        control_geometry: None,
                        dashed_alternative_arc: Some(SceneFilletAlternativeGeometry {
                            screen_polyline: alternative_model_polyline
                                .iter()
                                .copied()
                                .map(|point| viewport.model_to_screen(point))
                                .collect(),
                            model_polyline: alternative_model_polyline,
                        }),
                    },
                    SceneFilletAction {
                        id: SceneFilletActionId::ComplementaryArc,
                        owner: fixture.owner,
                        label: "Use complementary arc".into(),
                        availability: SceneFilletActionAvailability::Disabled {
                            reason: "Unavailable on the current branch".into(),
                        },
                        control_geometry: None,
                        dashed_alternative_arc: None,
                    },
                ],
            )
            .expect("test Fillet actions");
        (
            fixture
                .scene
                .fillet_action_target(
                    fixture.owner,
                    SceneFilletActionId::ReverseFirstRetainedDirection,
                )
                .expect("retained target"),
            fixture
                .scene
                .fillet_action_target(
                    fixture.owner,
                    SceneFilletActionId::LocalAlternative {
                        first: DocumentCurveNormalSide::Left,
                        second: DocumentCurveNormalSide::Right,
                    },
                )
                .expect("local target"),
        )
    }

    #[test]
    fn fillet_branch_canvas_and_accessible_inputs_share_preview_and_activation_authority() {
        let mut fixture = fillet_interaction_fixture(50.0, [2.0, 0.0]);
        let (retained, local) = install_test_fillet_actions(&mut fixture);
        let retained_position = fixture.scene.viewport.model_to_screen([2.0, -0.75]);
        let canvas = SceneFilletActionInput::Canvas {
            position: retained_position,
            painted: Some(retained),
        };
        let accessible = SceneFilletActionInput::Accessible(retained);
        assert_eq!(
            fixture
                .scene
                .resolve_fillet_action(canvas, PickTolerance::default()),
            Some(retained)
        );
        assert_eq!(
            fixture
                .scene
                .resolve_fillet_action(accessible, PickTolerance::default()),
            Some(retained)
        );

        let mut editor = ConstraintEditor::default();
        assert!(
            editor
                .activate_fillet_action(&fixture.scene, accessible)
                .is_empty()
        );
        assert_eq!(
            editor.preview_fillet_action(&fixture.scene, accessible),
            vec![EditorEffect::FilletBranchPreviewChanged {
                target: Some(retained),
            }]
        );
        assert_eq!(editor.fillet_branch_preview(), Some(retained));
        assert_eq!(
            editor.activate_fillet_action(&fixture.scene, accessible),
            vec![
                EditorEffect::CommitComputedFilletAction { target: retained },
                EditorEffect::FilletBranchPreviewChanged { target: None },
            ]
        );
        assert_eq!(editor.fillet_branch_preview(), None);

        assert_eq!(
            editor.preview_fillet_action(&fixture.scene, canvas),
            vec![EditorEffect::FilletBranchPreviewChanged {
                target: Some(retained),
            }]
        );
        assert_eq!(
            editor.pointer_leave(),
            vec![EditorEffect::FilletBranchPreviewChanged { target: None }],
            "leaving before the bubbled click must revoke preview authority",
        );
        assert!(
            editor
                .activate_fillet_action(&fixture.scene, canvas)
                .is_empty(),
            "canvas pointer-down must preserve the preview until click activation consumes it",
        );
        assert_eq!(
            editor.preview_fillet_action(&fixture.scene, canvas),
            vec![EditorEffect::FilletBranchPreviewChanged {
                target: Some(retained),
            }]
        );
        assert_eq!(
            editor.activate_fillet_action(&fixture.scene, canvas),
            vec![
                EditorEffect::CommitComputedFilletAction { target: retained },
                EditorEffect::FilletBranchPreviewChanged { target: None },
            ]
        );

        let local_position = fixture.scene.viewport.model_to_screen([-2.5, -3.25]);
        assert_eq!(
            fixture.scene.resolve_fillet_action(
                SceneFilletActionInput::Canvas {
                    position: local_position,
                    painted: Some(local),
                },
                PickTolerance::default(),
            ),
            Some(local),
            "alternative ghosts use their paired model-space polyline"
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn fillet_branch_resolution_rejects_spoofs_and_gives_painted_controls_precedence() {
        let mut fixture = fillet_interaction_fixture(50.0, [2.0, 0.0]);
        let (retained, _) = install_test_fillet_actions(&mut fixture);
        let retained_position = fixture.scene.viewport.model_to_screen([2.0, -0.75]);
        let far = fixture.scene.viewport.model_to_screen([-8.0, -8.0]);
        assert_eq!(
            fixture.scene.resolve_fillet_action(
                SceneFilletActionInput::Canvas {
                    position: far,
                    painted: Some(retained),
                },
                PickTolerance::default(),
            ),
            None,
            "a DOM target cannot replace independent proximity"
        );
        let foreign = SceneFilletActionTarget {
            owner: ComputedCornerRef {
                feature: retained.owner.feature,
                corner: ComputedFeatureCornerId::from_raw(retained.owner.corner.raw() + 1),
            },
            ..retained
        };
        assert_eq!(
            fixture.scene.resolve_fillet_action(
                SceneFilletActionInput::Canvas {
                    position: retained_position,
                    painted: Some(foreign),
                },
                PickTolerance::default(),
            ),
            None,
            "painted and independently resolved owners must agree"
        );
        let contact = fixture.scene.fillet_affordances[0].contacts[0].screen_position;
        assert!(matches!(
            fixture
                .scene
                .resolve_fillet_hit(contact, PickTolerance::default()),
            Some(SceneFilletHit::Radius { .. })
        ));
        assert_eq!(
            fixture.scene.resolve_fillet_action(
                SceneFilletActionInput::Canvas {
                    position: contact,
                    painted: Some(retained),
                },
                PickTolerance::default(),
            ),
            Some(retained),
            "an explicitly painted and independently verified arrow wins over the Fillet surface"
        );

        let disabled = fixture
            .scene
            .fillet_action_target(fixture.owner, SceneFilletActionId::ComplementaryArc)
            .expect("disabled action metadata remains stable");
        assert_eq!(
            fixture.scene.resolve_fillet_action(
                SceneFilletActionInput::Accessible(disabled),
                PickTolerance::default(),
            ),
            None
        );
        let mut stale = retained;
        stale.expected.features.revision =
            ComputedFeatureRevision::from_raw(stale.expected.features.revision.raw() + 1);
        assert_eq!(
            fixture.scene.resolve_fillet_action(
                SceneFilletActionInput::Accessible(stale),
                PickTolerance::default(),
            ),
            None
        );

        let mut editor = ConstraintEditor::default();
        editor.preview_fillet_action(&fixture.scene, SceneFilletActionInput::Accessible(retained));
        assert!(
            editor
                .activate_fillet_action(
                    &fixture.scene,
                    SceneFilletActionInput::Canvas {
                        position: retained_position,
                        painted: Some(foreign),
                    },
                )
                .is_empty()
        );
        assert_eq!(editor.fillet_branch_preview(), Some(retained));
    }

    #[test]
    fn fillet_branch_preview_clears_on_cancel_pointer_transition_and_stale_scene() {
        let mut fixture = fillet_interaction_fixture(50.0, [2.0, 0.0]);
        let (retained, _) = install_test_fillet_actions(&mut fixture);
        let accessible = SceneFilletActionInput::Accessible(retained);
        let mut editor = ConstraintEditor::default();

        editor.preview_fillet_action(&fixture.scene, accessible);
        assert_eq!(
            editor.cancel(),
            vec![EditorEffect::FilletBranchPreviewChanged { target: None }]
        );
        editor.preview_fillet_action(&fixture.scene, accessible);
        let ordinary = fixture.scene.viewport.model_to_screen([-8.0, -8.0]);
        assert_eq!(
            editor.pointer_down(
                &fixture.scene,
                pointer(91, ordinary.x, ordinary.y, Modifiers::default()),
            ),
            vec![EditorEffect::FilletBranchPreviewChanged { target: None }]
        );

        editor.preview_fillet_action(&fixture.scene, accessible);
        let stale = rebuilt_fillet_candidate_scene(&fixture);
        assert_eq!(
            editor.reconcile_fillet_branch_preview(&stale),
            vec![EditorEffect::FilletBranchPreviewChanged { target: None }]
        );
        assert_eq!(editor.fillet_branch_preview(), None);
    }

    #[test]
    fn fillet_branch_actions_and_live_radius_gesture_follow_geometry_policy() {
        let mut fixture = fillet_interaction_fixture(50.0, [2.0, 0.0]);
        let (retained, _) = install_test_fillet_actions(&mut fixture);
        fixture.scene.computed_curves[0].role = GeometryRole::Construction;
        let accessible = SceneFilletActionInput::Accessible(retained);
        let profile_policy = GeometryInteractionPolicy {
            scope: GeometryPickScope::Profile,
            visibility: GeometryVisibility::default(),
        };
        let construction_policy = GeometryInteractionPolicy {
            scope: GeometryPickScope::Construction,
            visibility: GeometryVisibility::default(),
        };
        let curve = &fixture.scene.computed_curves[0];
        assert!(curve.is_visible(profile_policy));
        assert!(!curve.is_interactive(profile_policy));
        assert!(curve.is_interactive(construction_policy));
        assert_eq!(
            fixture.scene.resolve_fillet_action_with_policy(
                accessible,
                PickTolerance::default(),
                profile_policy,
            ),
            None
        );
        assert_eq!(
            fixture.scene.resolve_fillet_action_with_policy(
                accessible,
                PickTolerance::default(),
                construction_policy,
            ),
            Some(retained)
        );

        let mut editor = ConstraintEditor::default();
        editor.set_geometry_pick_scope(GeometryPickScope::Construction);
        assert_eq!(
            editor.preview_fillet_action(&fixture.scene, accessible),
            vec![EditorEffect::FilletBranchPreviewChanged {
                target: Some(retained),
            }]
        );
        assert_eq!(
            editor.set_geometry_pick_scope(GeometryPickScope::Profile),
            vec![EditorEffect::FilletBranchPreviewChanged { target: None }]
        );
        assert!(
            editor
                .preview_fillet_action(&fixture.scene, accessible)
                .is_empty()
        );

        editor.set_geometry_pick_scope(GeometryPickScope::Construction);
        let rail = fixture.scene.fillet_affordances[0].radius_rail;
        let down = pointer(
            92,
            rail.screen_grip.x,
            rail.screen_grip.y,
            Modifiers::default(),
        );
        editor.pointer_down(&fixture.scene, down);
        assert_eq!(
            editor.active_pointer_gesture(),
            Some(ActivePointerGesture {
                pointer_id: 92,
                kind: ActivePointerGestureKind::FilletRadius,
            })
        );
        assert_eq!(
            editor.set_geometry_visibility(GeometryVisibility {
                explicit_construction: false,
                implicit_construction: true,
                reference_geometry: true,
            }),
            vec![EditorEffect::RestoreComputedFeatureRadius {
                expected: fixture.input,
                feature: fixture.owner.feature,
                radius: 2.0,
            }]
        );
        assert!(editor.active_pointer_gesture().is_none());
    }

    #[test]
    fn geometry_policy_transitions_cancel_prethreshold_point_press_and_retain_selection() {
        let (document, _, points) = line_document();
        let scene = scene(&document);
        let press = scene.viewport.model_to_screen(
            document
                .point(points[0])
                .expect("first line endpoint")
                .position,
        );
        let input = pointer(93, press.x, press.y, Modifiers::default());
        let mut editor = ConstraintEditor::default();

        let _ = editor.pointer_down(&scene, input);
        assert_eq!(
            editor.point_gesture_snapshot().map(|gesture| gesture.point),
            Some(points[0])
        );
        let selection = editor.selection().to_vec();
        let unchanged = editor.geometry_interaction_policy();
        assert!(editor.set_geometry_interaction_policy(unchanged).is_empty());
        assert!(
            editor.point_gesture_snapshot().is_some(),
            "an identical complete policy must not cancel a live press"
        );

        assert!(
            editor
                .set_geometry_pick_scope(GeometryPickScope::Profile)
                .is_empty(),
            "a pre-threshold press has no preview effect to clear"
        );
        assert!(editor.point_gesture_snapshot().is_none());
        assert_eq!(editor.selection(), selection);

        let _ = editor.pointer_down(&scene, input);
        assert!(editor.point_gesture_snapshot().is_some());
        assert!(
            editor
                .set_geometry_visibility(GeometryVisibility {
                    explicit_construction: false,
                    implicit_construction: true,
                    reference_geometry: true,
                })
                .is_empty(),
            "a visibility transition must still cancel before the drag threshold"
        );
        assert!(editor.point_gesture_snapshot().is_none());
        assert_eq!(editor.selection(), selection);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn fillet_affordances_validate_actions_and_expose_only_radius_canvas_handles() {
        let mut fixture = fillet_interaction_fixture(50.0, [2.0, 0.0]);
        let affordances = fixture
            .scene
            .fillet_affordances
            .first()
            .expect("affordances")
            .clone();
        assert_eq!(affordances.owner, fixture.owner);
        assert_eq!(affordances.affected_owners, vec![fixture.owner]);
        assert_eq!(affordances.contacts[0].source, fixture.sources[0]);
        assert_eq!(affordances.contacts[1].source, fixture.sources[1]);
        assert!(matches!(
            fixture.scene.resolve_fillet_hit(
                affordances.contacts[0].screen_position,
                PickTolerance::default()
            ),
            Some(SceneFilletHit::Radius { owner, .. }) if owner == fixture.owner
        ));
        assert!(matches!(
            fixture.scene.resolve_fillet_hit(
                affordances.radius_rail.screen_rail_start,
                PickTolerance::default()
            ),
            Some(SceneFilletHit::Radius { owner, .. }) if owner == fixture.owner
        ));
        let native = fixture.scene.viewport.model_to_screen([2.0, -2.0]);
        assert!(matches!(
            fixture
                .scene
                .resolve_fillet_hit(native, PickTolerance::default()),
            Some(SceneFilletHit::Native(Hit {
                item: SelectionItem::Curve(span),
                ..
            })) if span == fixture.sources[0].span
        ));

        let retained_direction_geometry = SceneFilletActionControlGeometry {
            model_anchor: [2.0, 0.0],
            model_direction: [0.0, 1.0],
            screen_start: ScreenPoint { x: 600.0, y: 350.0 },
            screen_end: ScreenPoint { x: 600.0, y: 330.0 },
        };
        let alternative_model_polyline = vec![[-8.0, 6.0], [-4.0, 5.0]];
        let alternative_geometry = SceneFilletAlternativeGeometry {
            screen_polyline: alternative_model_polyline
                .iter()
                .copied()
                .map(|point| fixture.scene.viewport.model_to_screen(point))
                .collect(),
            model_polyline: alternative_model_polyline,
        };
        let actions = vec![
            SceneFilletAction {
                id: SceneFilletActionId::ReverseFirstRetainedDirection,
                owner: fixture.owner,
                label: "Reverse first retained direction".into(),
                availability: SceneFilletActionAvailability::Applicable,
                control_geometry: Some(retained_direction_geometry),
                dashed_alternative_arc: None,
            },
            SceneFilletAction {
                id: SceneFilletActionId::ComplementaryArc,
                owner: fixture.owner,
                label: "Use complementary arc".into(),
                availability: SceneFilletActionAvailability::Disabled {
                    reason: "No current local alternative".into(),
                },
                control_geometry: None,
                dashed_alternative_arc: Some(alternative_geometry),
            },
        ];
        fixture
            .scene
            .set_fillet_corner_actions(fixture.owner, actions.clone())
            .expect("valid actions");
        assert_eq!(fixture.scene.fillet_affordances[0].actions, actions);
        assert_eq!(
            fixture.scene.fillet_affordances[0].actions[0].control_geometry,
            Some(retained_direction_geometry)
        );
        let mut invalid_control = actions.clone();
        invalid_control[0].control_geometry = Some(SceneFilletActionControlGeometry {
            model_direction: [0.0, 0.0],
            ..retained_direction_geometry
        });
        assert!(matches!(
            fixture
                .scene
                .set_fillet_corner_actions(fixture.owner, invalid_control),
            Err(EditorError::InvalidComputedFeatureAffordance)
        ));
        assert_eq!(fixture.scene.fillet_affordances[0].actions, actions);
        let mut mismatched_alternative = actions.clone();
        mismatched_alternative[1]
            .dashed_alternative_arc
            .as_mut()
            .expect("alternative geometry")
            .screen_polyline[0]
            .x += 1.0;
        assert!(matches!(
            fixture
                .scene
                .set_fillet_corner_actions(fixture.owner, mismatched_alternative),
            Err(EditorError::InvalidComputedFeatureAffordance)
        ));
        assert_eq!(fixture.scene.fillet_affordances[0].actions, actions);
        let duplicate = vec![
            SceneFilletAction {
                id: SceneFilletActionId::LocalAlternative {
                    first: DocumentCurveNormalSide::Left,
                    second: DocumentCurveNormalSide::Right,
                },
                owner: fixture.owner,
                label: "First".into(),
                availability: SceneFilletActionAvailability::Applicable,
                control_geometry: None,
                dashed_alternative_arc: None,
            },
            SceneFilletAction {
                id: SceneFilletActionId::LocalAlternative {
                    first: DocumentCurveNormalSide::Left,
                    second: DocumentCurveNormalSide::Right,
                },
                owner: fixture.owner,
                label: "Duplicate".into(),
                availability: SceneFilletActionAvailability::Applicable,
                control_geometry: None,
                dashed_alternative_arc: None,
            },
        ];
        assert!(matches!(
            fixture
                .scene
                .set_fillet_corner_actions(fixture.owner, duplicate),
            Err(EditorError::InvalidComputedFeatureAffordance)
        ));
        assert_eq!(fixture.scene.fillet_affordances[0].actions, actions);
        let later_owner = ComputedCornerRef {
            feature: fixture.owner.feature,
            corner: ComputedFeatureCornerId::from_raw(fixture.owner.corner.raw() + 1),
        };
        for malformed_owners in [
            Vec::new(),
            vec![fixture.owner, fixture.owner],
            vec![later_owner, fixture.owner],
            vec![later_owner],
        ] {
            assert!(matches!(
                fixture.scene.attach_computed_fillet_radius_rail(
                    fixture.owner,
                    [2.0, 0.0],
                    malformed_owners
                ),
                Err(EditorError::InvalidComputedFeatureAffordance)
            ));
        }
        assert_eq!(
            fixture.scene.fillet_affordances[0].affected_owners,
            vec![fixture.owner]
        );
        assert!(matches!(
            fixture.scene.attach_computed_fillet_radius_rail(
                fixture.owner,
                [0.0, 0.0],
                vec![fixture.owner]
            ),
            Err(EditorError::StaleComputedFeatureSnapshot)
        ));
        let curve = &fixture.scene.computed_curves[0];
        let alternative = ComputedCircularArc {
            center: curve.center,
            radius: curve.radius,
            start_angle: curve.start_angle,
            end_angle: curve.end_angle,
            sweep: curve.sweep,
            contacts: curve.contacts,
        };
        assert!(
            fixture
                .scene
                .tessellate_computed_fillet_arc(&alternative, 0.25)
                .is_ok_and(|geometry| {
                    geometry.model_polyline.len() >= 2
                        && geometry.model_polyline.len() == geometry.screen_polyline.len()
                })
        );
        assert!(matches!(
            fixture
                .scene
                .tessellate_computed_fillet_arc(&alternative, 0.0),
            Err(EditorError::InvalidTolerance)
        ));
        let overflowing = ComputedCircularArc {
            center: [f64::MAX, 0.0],
            radius: f64::MAX,
            start_angle: 0.0,
            end_angle: std::f64::consts::FRAC_PI_2,
            ..alternative
        };
        assert!(matches!(
            fixture
                .scene
                .tessellate_computed_fillet_arc(&overflowing, 0.25),
            Err(EditorError::StaleComputedFeatureSnapshot)
        ));
    }

    #[test]
    fn radius_rail_maps_radial_motion_ignores_tangential_motion_and_keeps_exact_ack() {
        let fixture = fillet_interaction_fixture(50.0, [2.0, 0.0]);
        let rail = fixture.scene.fillet_affordances[0].radius_rail;
        let mut editor = ConstraintEditor::default();
        let down = editor.pointer_down(
            &fixture.scene,
            pointer(
                17,
                rail.screen_grip.x,
                rail.screen_grip.y,
                Modifiers {
                    shift: true,
                    ..Modifiers::default()
                },
            ),
        );
        assert!(matches!(
            down.as_slice(),
            [EditorEffect::SelectionChanged(selection)]
                if selection == &[SelectionItem::FeatureCorner(fixture.owner)]
        ));
        assert_eq!(
            editor.active_pointer_gesture(),
            Some(ActivePointerGesture {
                pointer_id: 17,
                kind: ActivePointerGestureKind::FilletRadius,
            })
        );
        assert!(
            editor
                .pointer_move(
                    &fixture.scene,
                    pointer(
                        17,
                        rail.screen_grip.x,
                        rail.screen_grip.y + 20.0,
                        Modifiers::default(),
                    ),
                )
                .is_empty(),
            "motion perpendicular to dC/dr must not change radius"
        );
        let moved = pointer(
            17,
            rail.screen_grip.x + 100.0,
            rail.screen_grip.y,
            Modifiers::default(),
        );
        let radius = preview_radius(&editor.pointer_move(&fixture.scene, moved));
        assert!((radius - 3.0).abs() < 1.0e-12);
        assert!(editor.accept_computed_feature_radius_preview(
            &fixture.input,
            fixture.owner.feature,
            radius
        ));
        assert!(editor.pointer_move(&fixture.scene, moved).is_empty());
        assert!(matches!(
            editor
                .pointer_up(&fixture.scene, fixture.scene.design_identity, moved)
                .as_slice(),
            [
                EditorEffect::CommitComputedFeatureRadius { radius: committed, .. },
                EditorEffect::ClearComputedFeaturePreview,
            ] if committed.to_bits() == radius.to_bits()
        ));
        assert_eq!(editor.active_pointer_gesture(), None);
    }

    #[test]
    fn fillet_endpoint_hover_and_pointer_down_use_the_visible_radius_surface() {
        let fixture = fillet_interaction_fixture(50.0, [2.0, 0.0]);
        let affordances = &fixture.scene.fillet_affordances[0];
        let mut editor = ConstraintEditor::default();
        let contact = affordances.contacts[0].screen_position;
        assert!(matches!(
            editor
                .pointer_move(
                    &fixture.scene,
                    pointer(61, contact.x, contact.y, Modifiers::default())
                )
                .as_slice(),
            [EditorEffect::HoverChanged(EditorHoverState {
                target: Some(EditorHoverTarget::Geometry(SelectionItem::FeatureCorner(owner))),
                ..
            })] if *owner == fixture.owner
        ));
        editor.pointer_down(
            &fixture.scene,
            pointer(61, contact.x, contact.y, Modifiers::default()),
        );
        assert_eq!(
            editor.active_pointer_gesture(),
            Some(ActivePointerGesture {
                pointer_id: 61,
                kind: ActivePointerGestureKind::FilletRadius,
            })
        );
        editor.cancel();

        let radius = affordances.radius_rail.screen_rail_start;
        assert!(matches!(
            editor
                .pointer_move(
                    &fixture.scene,
                    pointer(62, radius.x, radius.y, Modifiers::default())
                )
                .as_slice(),
            [EditorEffect::HoverChanged(EditorHoverState {
                target: Some(EditorHoverTarget::Geometry(SelectionItem::FeatureCorner(owner))),
                ..
            })] if *owner == fixture.owner
        ));
        editor.pointer_down(
            &fixture.scene,
            pointer(62, radius.x, radius.y, Modifiers::default()),
        );
        assert_eq!(
            editor.active_pointer_gesture(),
            Some(ActivePointerGesture {
                pointer_id: 62,
                kind: ActivePointerGestureKind::FilletRadius,
            })
        );
    }

    #[test]
    fn radius_release_requires_latest_current_sample_and_rejects_foreign_pointer_or_owner() {
        let fixture = fillet_interaction_fixture(50.0, [2.0, 0.0]);
        let rail = fixture.scene.fillet_affordances[0].radius_rail;
        let down = pointer(
            23,
            rail.screen_grip.x,
            rail.screen_grip.y,
            Modifiers::default(),
        );
        let mut editor = ConstraintEditor::default();
        editor.pointer_down(&fixture.scene, down);
        let first_move = pointer(
            23,
            rail.screen_grip.x + 50.0,
            rail.screen_grip.y,
            Modifiers::default(),
        );
        let accepted = preview_radius(&editor.pointer_move(&fixture.scene, first_move));
        assert!(editor.accept_computed_feature_radius_preview(
            &fixture.input,
            fixture.owner.feature,
            accepted
        ));
        assert!(
            editor
                .pointer_down(
                    &fixture.scene,
                    pointer(
                        24,
                        rail.screen_grip.x,
                        rail.screen_grip.y,
                        Modifiers::default(),
                    )
                )
                .is_empty()
        );
        assert!(
            editor
                .pointer_move(&fixture.scene, pointer(24, 0.0, 0.0, Modifiers::default()))
                .is_empty()
        );
        assert!(
            editor
                .pointer_up(
                    &fixture.scene,
                    fixture.scene.design_identity,
                    pointer(24, 0.0, 0.0, Modifiers::default())
                )
                .is_empty()
        );
        assert_eq!(
            editor
                .active_pointer_gesture()
                .map(|gesture| gesture.pointer_id),
            Some(23)
        );

        let invalid = pointer(
            23,
            rail.screen_grip.x - 300.0,
            rail.screen_grip.y,
            Modifiers::default(),
        );
        assert!(editor.pointer_move(&fixture.scene, invalid).is_empty());
        assert_eq!(
            editor.pointer_up(&fixture.scene, fixture.scene.design_identity, invalid),
            vec![EditorEffect::ClearComputedFeaturePreview],
            "an earlier accepted sample must not publish after a later invalid sample"
        );

        let foreign = ComputedCornerRef {
            feature: fixture.owner.feature,
            corner: ComputedFeatureCornerId::from_raw(fixture.owner.corner.raw() + 1),
        };
        assert!(
            editor
                .pointer_down_feature_radius(
                    &fixture.scene,
                    down,
                    foreign,
                    PickTolerance::default()
                )
                .is_none()
        );
        editor.pointer_down(&fixture.scene, down);
        let moved = preview_radius(&editor.pointer_move(&fixture.scene, first_move));
        assert!(!editor.accept_computed_feature_radius_preview(
            &fixture.input,
            ComputedFeatureId::from_raw(fixture.owner.feature.raw() + 1),
            moved
        ));
        assert_eq!(
            editor.cancel(),
            vec![EditorEffect::RestoreComputedFeatureRadius {
                expected: fixture.input,
                feature: fixture.owner.feature,
                radius: 2.0,
            }]
        );
    }

    #[test]
    fn painted_radius_fallback_respects_computed_curve_pick_scope() {
        let mut fixture = fillet_interaction_fixture(50.0, [2.0, 0.0]);
        fixture.scene.fillet_affordances.clear();
        fixture.scene.computed_curves[0].role = GeometryRole::Construction;
        let position = fixture.scene.viewport.model_to_screen([
            2.0 * std::f64::consts::FRAC_1_SQRT_2,
            2.0 * std::f64::consts::FRAC_1_SQRT_2,
        ]);
        let down = pointer(25, position.x, position.y, Modifiers::default());
        let mut editor = ConstraintEditor::default();
        editor.set_geometry_pick_scope(GeometryPickScope::Profile);

        assert!(
            editor
                .pointer_down_feature_radius(
                    &fixture.scene,
                    down,
                    fixture.owner,
                    PickTolerance::default(),
                )
                .is_none(),
            "a painted owner cannot bypass a scope that excludes its computed curve"
        );
        assert!(editor.active_pointer_gesture().is_none());

        editor.set_geometry_pick_scope(GeometryPickScope::Construction);
        assert!(
            editor
                .pointer_down_feature_radius(
                    &fixture.scene,
                    down,
                    fixture.owner,
                    PickTolerance::default(),
                )
                .is_some(),
            "the same exact fallback remains available in Construction scope"
        );
        assert_eq!(
            editor.active_pointer_gesture(),
            Some(ActivePointerGesture {
                pointer_id: 25,
                kind: ActivePointerGestureKind::FilletRadius,
            })
        );
    }

    #[test]
    fn typed_radius_limit_is_exact_clears_on_recovery_and_cannot_publish_on_release() {
        let fixture = fillet_interaction_fixture(50.0, [2.0, 0.0]);
        let rail = fixture.scene.fillet_affordances[0].radius_rail;
        let mut editor = ConstraintEditor::default();
        let down = pointer(
            81,
            rail.screen_grip.x,
            rail.screen_grip.y,
            Modifiers::default(),
        );
        editor.pointer_down(&fixture.scene, down);
        let first = pointer(
            81,
            rail.screen_grip.x + 50.0,
            rail.screen_grip.y,
            Modifiers::default(),
        );
        let first_radius = preview_radius(&editor.pointer_move(&fixture.scene, first));
        let fold = ComputedFilletContinuationLimit {
            kind: ComputedFilletContinuationLimitKind::BranchFold,
            message: "The selected local root ends at this fold".into(),
        };
        assert!(editor.reject_computed_feature_radius_preview(
            &fixture.input,
            fixture.owner.feature,
            first_radius,
            fold.clone(),
        ));
        assert_eq!(
            editor.computed_fillet_continuation_status(),
            Some(&ComputedFilletContinuationStatus {
                expected: fixture.input,
                owner: fixture.owner,
                sample: ComputedFilletInteractionSample::Radius(first_radius),
                limit: fold.clone(),
            })
        );
        assert!(!editor.reject_computed_feature_radius_preview(
            &fixture.input,
            fixture.owner.feature,
            first_radius + 1.0,
            fold.clone(),
        ));
        assert!(
            !editor.accept_computed_feature_radius_preview(
                &fixture.input,
                fixture.owner.feature,
                first_radius,
            ),
            "a rejected sample consumes its acknowledgement token"
        );
        let retried_radius = preview_radius(&editor.pointer_move(&fixture.scene, first));
        assert_eq!(retried_radius.to_bits(), first_radius.to_bits());
        assert!(editor.accept_computed_feature_radius_preview(
            &fixture.input,
            fixture.owner.feature,
            retried_radius,
        ));
        assert_eq!(editor.computed_fillet_continuation_status(), None);

        let second = pointer(
            81,
            rail.screen_grip.x + 100.0,
            rail.screen_grip.y,
            Modifiers::default(),
        );
        let second_radius = preview_radius(&editor.pointer_move(&fixture.scene, second));
        assert!(editor.reject_computed_feature_radius_preview(
            &fixture.input,
            fixture.owner.feature,
            second_radius,
            fold,
        ));
        assert_eq!(
            editor.pointer_up(&fixture.scene, fixture.scene.design_identity, second),
            vec![EditorEffect::ClearComputedFeaturePreview],
        );
        assert_eq!(editor.computed_fillet_continuation_status(), None);
    }

    #[test]
    fn scene_exposes_typed_fold_status_without_inventing_a_radius_rail() {
        let fixture = fillet_interaction_fixture(50.0, [2.0, 0.0]);
        let status = ComputedFilletContinuationStatus {
            expected: fixture.input,
            owner: fixture.owner,
            sample: ComputedFilletInteractionSample::Radius(0.5),
            limit: ComputedFilletContinuationLimit {
                kind: ComputedFilletContinuationLimitKind::BranchFold,
                message: "Radius rail is ill-conditioned at the current fold".into(),
            },
        };
        let mut scene = fixture.scene.clone();
        scene.fillet_affordances.clear();
        scene
            .set_computed_fillet_continuation_status(fixture.owner, Some(status.clone()))
            .expect("top-level fold status without rail");
        assert!(scene.fillet_affordances.is_empty());
        assert_eq!(scene.computed_fillet_continuation_statuses, vec![status]);
    }

    #[test]
    fn radius_projection_is_zoom_and_sample_count_invariant() {
        fn requested_radius(pixels_per_model_unit: f64, intermediate_samples: usize) -> f64 {
            let fixture = fillet_interaction_fixture(pixels_per_model_unit, [2.0, 0.0]);
            let rail = fixture.scene.fillet_affordances[0].radius_rail;
            let mut editor = ConstraintEditor::default();
            editor.pointer_down(
                &fixture.scene,
                pointer(
                    31,
                    rail.screen_grip.x,
                    rail.screen_grip.y,
                    Modifiers::default(),
                ),
            );
            let sample_denominator = f64::from(
                u32::try_from(intermediate_samples + 1).expect("bounded test sample count"),
            );
            for sample in 1..=intermediate_samples {
                let fraction = f64::from(u32::try_from(sample).expect("bounded test sample"))
                    / sample_denominator;
                let position = fixture
                    .scene
                    .viewport
                    .model_to_screen([rail.model_grip[0] + 2.0 * fraction, rail.model_grip[1]]);
                assert!(matches!(
                    editor
                        .pointer_move(
                            &fixture.scene,
                            pointer(31, position.x, position.y, Modifiers::default())
                        )
                        .as_slice(),
                    [EditorEffect::PreviewComputedFeatureRadius { .. }]
                ));
            }
            let final_position = fixture
                .scene
                .viewport
                .model_to_screen([rail.model_grip[0] + 2.0, rail.model_grip[1]]);
            preview_radius(&editor.pointer_move(
                &fixture.scene,
                pointer(31, final_position.x, final_position.y, Modifiers::default()),
            ))
        }

        let coarse = requested_radius(25.0, 0);
        let fine = requested_radius(25.0, 7);
        let zoomed = requested_radius(125.0, 3);
        assert!((coarse - 3.0).abs() < 1.0e-12);
        assert_eq!(coarse.to_bits(), fine.to_bits());
        assert!((coarse - zoomed).abs() < 1.0e-12);
    }

    #[test]
    fn rebuilt_current_scene_keeps_the_exact_radius_gesture_origin() {
        let fixture = fillet_interaction_fixture(50.0, [2.0, 0.0]);
        let rail = fixture.scene.fillet_affordances[0].radius_rail;
        let mut editor = ConstraintEditor::default();
        editor.pointer_down(
            &fixture.scene,
            pointer(
                71,
                rail.screen_grip.x,
                rail.screen_grip.y,
                Modifiers::default(),
            ),
        );
        let first = pointer(
            71,
            rail.screen_grip.x + 50.0,
            rail.screen_grip.y,
            Modifiers::default(),
        );
        let first_radius = preview_radius(&editor.pointer_move(&fixture.scene, first));
        assert!(editor.accept_computed_feature_radius_preview(
            &fixture.input,
            fixture.owner.feature,
            first_radius
        ));

        let mut rebuilt = rebuilt_fillet_candidate_scene(&fixture);
        let mut foreign_origin = fixture.input;
        foreign_origin.features.document =
            ComputedFeatureDocumentId::from_raw(foreign_origin.features.document.raw() + 1);
        assert!(matches!(
            rebuilt.set_computed_fillet_interaction_origin(foreign_origin),
            Err(EditorError::StaleComputedFeatureSnapshot)
        ));
        rebuilt
            .set_computed_fillet_interaction_origin(fixture.input)
            .expect("matching interaction origin");
        let second = pointer(
            71,
            rail.screen_grip.x + 100.0,
            rail.screen_grip.y,
            Modifiers::default(),
        );
        let second_radius = preview_radius(&editor.pointer_move(&rebuilt, second));
        assert!((second_radius - 3.0).abs() < 1.0e-12);
        assert!(editor.accept_computed_feature_radius_preview(
            &fixture.input,
            fixture.owner.feature,
            second_radius
        ));
        assert!(matches!(
            editor
                .pointer_up(&rebuilt, rebuilt.design_identity, second)
                .as_slice(),
            [EditorEffect::CommitComputedFeatureRadius { expected, radius, .. }, EditorEffect::ClearComputedFeaturePreview]
                if *expected == fixture.input && radius.to_bits() == second_radius.to_bits()
        ));
    }

    #[test]
    fn rebuilt_current_scene_keeps_the_exact_contact_gesture_origin() {
        let fixture = fillet_interaction_fixture(50.0, [2.0, 0.0]);
        let handle = fixture.scene.fillet_affordances[0].contacts[0];
        let mut editor = ConstraintEditor::default();
        editor.pointer_down_feature_contact_handle(
            &fixture.scene,
            pointer(
                72,
                handle.screen_position.x,
                handle.screen_position.y,
                Modifiers::default(),
            ),
            handle,
        );
        let first_target = fixture.scene.viewport.model_to_screen([2.0, 1.0]);
        let first = pointer(72, first_target.x, first_target.y, Modifiers::default());
        let first_parameter =
            preview_contact_parameter(&editor.pointer_move(&fixture.scene, first));
        assert!(editor.accept_computed_feature_contact_preview(
            &fixture.input,
            fixture.owner,
            ComputedFilletParentIndex::First,
            fixture.sources[0],
            first_parameter,
        ));

        let mut rebuilt = rebuilt_fillet_candidate_scene(&fixture);
        rebuilt
            .set_computed_fillet_interaction_origin(fixture.input)
            .expect("matching interaction origin");
        let second_target = fixture.scene.viewport.model_to_screen([2.0, 2.0]);
        let second = pointer(72, second_target.x, second_target.y, Modifiers::default());
        let second_parameter = preview_contact_parameter(&editor.pointer_move(&rebuilt, second));
        assert!((second_parameter - 0.75).abs() < 1.0e-12);
        assert!(editor.accept_computed_feature_contact_preview(
            &fixture.input,
            fixture.owner,
            ComputedFilletParentIndex::First,
            fixture.sources[0],
            second_parameter,
        ));
        assert!(matches!(
            editor
                .pointer_up(&rebuilt, rebuilt.design_identity, second)
                .as_slice(),
            [EditorEffect::CommitComputedFeatureContact { expected, parameter, .. }, EditorEffect::ClearComputedFeatureContactPreview]
                if *expected == fixture.input && parameter.to_bits() == second_parameter.to_bits()
        ));
    }

    #[test]
    fn contact_gesture_projects_only_named_parent_and_requires_exact_ack() {
        let fixture = fillet_interaction_fixture(50.0, [2.0, 0.0]);
        let handle = fixture.scene.fillet_affordances[0].contacts[0];
        let mut editor = ConstraintEditor::default();
        assert!(matches!(
            editor
                .pointer_down_feature_contact_handle(
                    &fixture.scene,
                    pointer(
                        41,
                        handle.screen_position.x,
                        handle.screen_position.y,
                        Modifiers::default(),
                    ),
                    handle,
                )
                .as_slice(),
            [EditorEffect::SelectionChanged(selection)]
                if selection == &[SelectionItem::FeatureCorner(fixture.owner)]
        ));
        assert_eq!(
            editor.active_pointer_gesture(),
            Some(ActivePointerGesture {
                pointer_id: 41,
                kind: ActivePointerGestureKind::FilletContact,
            })
        );
        let target = fixture.scene.viewport.model_to_screen([2.0, 2.0]);
        let moved = pointer(41, target.x, target.y, Modifiers::default());
        let parameter = preview_contact_parameter(&editor.pointer_move(&fixture.scene, moved));
        assert!((parameter - 0.75).abs() < 1.0e-12);
        assert!(!editor.accept_computed_feature_contact_preview(
            &fixture.input,
            fixture.owner,
            ComputedFilletParentIndex::Second,
            fixture.sources[0],
            parameter,
        ));
        assert!(editor.accept_computed_feature_contact_preview(
            &fixture.input,
            fixture.owner,
            ComputedFilletParentIndex::First,
            fixture.sources[0],
            parameter,
        ));
        assert!(matches!(
            editor
                .pointer_up(&fixture.scene, fixture.scene.design_identity, moved)
                .as_slice(),
            [
                EditorEffect::CommitComputedFeatureContact {
                    owner,
                    parent: ComputedFilletParentIndex::First,
                    source,
                    parameter: committed,
                    ..
                },
                EditorEffect::ClearComputedFeatureContactPreview,
            ] if *owner == fixture.owner
                && *source == fixture.sources[0]
                && committed.to_bits() == parameter.to_bits()
        ));
    }

    #[test]
    fn typed_contact_limit_requires_the_exact_named_parent_sample() {
        let fixture = fillet_interaction_fixture(50.0, [2.0, 0.0]);
        let handle = fixture.scene.fillet_affordances[0].contacts[0];
        let mut editor = ConstraintEditor::default();
        editor.pointer_down_feature_contact_handle(
            &fixture.scene,
            pointer(
                82,
                handle.screen_position.x,
                handle.screen_position.y,
                Modifiers::default(),
            ),
            handle,
        );
        let target = fixture.scene.viewport.model_to_screen([2.0, 2.0]);
        let moved = pointer(82, target.x, target.y, Modifiers::default());
        let parameter = preview_contact_parameter(&editor.pointer_move(&fixture.scene, moved));
        let domain = ComputedFilletContinuationLimit {
            kind: ComputedFilletContinuationLimitKind::DomainBoundary,
            message: "The named parent contact reached its retained domain".into(),
        };
        assert!(!editor.reject_computed_feature_contact_preview(
            &fixture.input,
            fixture.owner,
            ComputedFilletParentIndex::Second,
            fixture.sources[0],
            parameter,
            domain.clone(),
        ));
        assert_eq!(editor.computed_fillet_continuation_status(), None);
        assert!(editor.reject_computed_feature_contact_preview(
            &fixture.input,
            fixture.owner,
            ComputedFilletParentIndex::First,
            fixture.sources[0],
            parameter,
            domain.clone(),
        ));
        assert_eq!(
            editor.computed_fillet_continuation_status(),
            Some(&ComputedFilletContinuationStatus {
                expected: fixture.input,
                owner: fixture.owner,
                sample: ComputedFilletInteractionSample::Contact {
                    parent: ComputedFilletParentIndex::First,
                    source: fixture.sources[0],
                    parameter,
                },
                limit: domain,
            })
        );
        assert!(
            !editor.accept_computed_feature_contact_preview(
                &fixture.input,
                fixture.owner,
                ComputedFilletParentIndex::First,
                fixture.sources[0],
                parameter,
            ),
            "a rejected contact sample consumes its acknowledgement token"
        );
        let retried_parameter =
            preview_contact_parameter(&editor.pointer_move(&fixture.scene, moved));
        assert_eq!(retried_parameter.to_bits(), parameter.to_bits());
        assert!(editor.accept_computed_feature_contact_preview(
            &fixture.input,
            fixture.owner,
            ComputedFilletParentIndex::First,
            fixture.sources[0],
            retried_parameter,
        ));
        assert_eq!(editor.computed_fillet_continuation_status(), None);
    }

    #[test]
    fn contact_cancel_second_pointer_and_invalid_release_are_state_neutral() {
        let fixture = fillet_interaction_fixture(50.0, [2.0, 0.0]);
        let handle = fixture.scene.fillet_affordances[0].contacts[1];
        let down = pointer(
            51,
            handle.screen_position.x,
            handle.screen_position.y,
            Modifiers::default(),
        );
        let mut editor = ConstraintEditor::default();
        editor.pointer_down_feature_contact_handle(&fixture.scene, down, handle);
        assert!(
            editor
                .pointer_down(
                    &fixture.scene,
                    pointer(
                        52,
                        handle.screen_position.x,
                        handle.screen_position.y,
                        Modifiers::default(),
                    )
                )
                .is_empty()
        );
        let target = fixture.scene.viewport.model_to_screen([2.0, 2.0]);
        let moved = pointer(51, target.x, target.y, Modifiers::default());
        let parameter = preview_contact_parameter(&editor.pointer_move(&fixture.scene, moved));
        assert!(editor.accept_computed_feature_contact_preview(
            &fixture.input,
            fixture.owner,
            ComputedFilletParentIndex::Second,
            fixture.sources[1],
            parameter,
        ));
        let mut stale = fixture.scene.clone();
        stale.computed_input = None;
        assert!(editor.pointer_move(&stale, moved).is_empty());
        assert_eq!(
            editor.pointer_up(&fixture.scene, fixture.scene.design_identity, moved),
            vec![EditorEffect::ClearComputedFeatureContactPreview]
        );

        editor.pointer_down_feature_contact_handle(&fixture.scene, down, handle);
        let parameter = preview_contact_parameter(&editor.pointer_move(&fixture.scene, moved));
        assert!(editor.accept_computed_feature_contact_preview(
            &fixture.input,
            fixture.owner,
            ComputedFilletParentIndex::Second,
            fixture.sources[1],
            parameter,
        ));
        let unsampled = fixture.scene.viewport.model_to_screen([3.0, 2.0]);
        assert_eq!(
            editor.pointer_up(
                &fixture.scene,
                fixture.scene.design_identity,
                pointer(51, unsampled.x, unsampled.y, Modifiers::default())
            ),
            vec![EditorEffect::ClearComputedFeatureContactPreview],
            "release must match the exact acknowledged contact sample"
        );

        editor.pointer_down_feature_contact_handle(&fixture.scene, down, handle);
        assert_eq!(
            editor.cancel(),
            vec![EditorEffect::RestoreComputedFeatureContact {
                expected: fixture.input,
                owner: fixture.owner,
                parent: ComputedFilletParentIndex::Second,
                source: fixture.sources[1],
                parameter: 0.5,
            }]
        );
        assert_eq!(editor.active_pointer_gesture(), None);
    }

    fn pointer(pointer_id: u64, x: f64, y: f64, modifiers: Modifiers) -> PointerInput {
        PointerInput {
            pointer_id,
            position: ScreenPoint { x, y },
            modifiers,
        }
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the retained M63 fixture verifies annotation visibility, occurrence picking, and geometry ownership together"
    )]
    fn m63_annotations_are_geometry_anchored_contextual_and_pointer_selectable() {
        let mut document = SketchDocument::new(8.0).expect("document");
        let rectangle = document
            .add_rectangle("annotated", [0.0, 0.0], 4.0, 3.0)
            .expect("rectangle");
        document
            .set_dimension_mode(rectangle.dimensions[1], DocumentDimensionMode::Reference)
            .expect("reference dimension");
        let duplicate = document
            .add_constraint(
                "duplicate horizontal",
                DocumentConstraintDefinition::Horizontal {
                    line: CurveSpan::line(rectangle.curves[0]),
                },
            )
            .expect("duplicate constraint");
        let scene = scene(&document);
        let horizontal_annotation = scene
            .annotations
            .iter()
            .find(|annotation| {
                annotation.kind == SceneAnnotationKind::Constraint(SceneConstraintGlyph::Horizontal)
            })
            .expect("horizontal annotation");
        let horizontal = horizontal_annotation.item;
        assert_eq!(
            horizontal_annotation.visibility,
            SceneAnnotationVisibility::Contextual
        );
        let related_curve = horizontal_annotation
            .operands
            .iter()
            .find_map(|item| match item {
                SelectionItem::Curve(span) => Some(*span),
                _ => None,
            })
            .expect("constraint curve operand");
        assert!(!horizontal_annotation.is_visible(&[], None, &[]));
        assert!(horizontal_annotation.is_visible(
            &[SelectionItem::Curve(related_curve)],
            None,
            &[]
        ));
        let driving = scene
            .annotations
            .iter()
            .find(|annotation| annotation.item == SelectionItem::Dimension(rectangle.dimensions[0]))
            .expect("driving dimension");
        let reference = scene
            .annotations
            .iter()
            .find(|annotation| annotation.item == SelectionItem::Dimension(rectangle.dimensions[1]))
            .expect("reference dimension");
        assert_eq!(driving.visibility, SceneAnnotationVisibility::Always);
        assert_eq!(reference.visibility, SceneAnnotationVisibility::Contextual);
        let duplicate_annotation = scene
            .annotations
            .iter()
            .find(|annotation| annotation.item == SelectionItem::Constraint(duplicate))
            .expect("duplicate annotation");
        assert!(matches!(
            &duplicate_annotation.geometry,
            SceneAnnotationGeometry::Glyph { markers }
                if markers.iter().any(|marker| marker.leader_from.is_some())
        ));
        let dimension_bounds = scene
            .annotations
            .iter()
            .filter_map(|candidate| candidate.label_bounds)
            .collect::<Vec<_>>();
        for marker in scene.annotations.iter().flat_map(|candidate| {
            if let SceneAnnotationGeometry::Glyph { markers } = &candidate.geometry {
                markers.as_slice()
            } else {
                &[]
            }
        }) {
            assert!(
                marker.anchor.x >= 14.0
                    && marker.anchor.y >= 14.0
                    && marker.anchor.x <= scene.viewport.screen_size[0] - 14.0
                    && marker.anchor.y <= scene.viewport.screen_size[1] - 14.0
            );
            assert!(
                dimension_bounds
                    .iter()
                    .all(|bounds| bounds.distance(marker.anchor) >= 14.0),
                "automatic glyph bounds must reserve visible dimension labels"
            );
        }

        let marker = match &horizontal_annotation.geometry {
            SceneAnnotationGeometry::Glyph { markers } => markers[0].anchor,
            _ => panic!("horizontal must be a glyph"),
        };
        assert!(
            scene
                .annotation_hit_test(marker, PickTolerance::default(), &[], None, &[])
                .is_none(),
            "contextual marks must not gain an invisible hit target by default"
        );
        let mut show_all_scene = scene.clone();
        show_all_scene.set_show_all_constraint_annotations(true);
        assert_eq!(
            show_all_scene
                .annotation_hit_test(marker, PickTolerance::default(), &[], None, &[])
                .map(|hit| hit.item),
            Some(horizontal),
            "Display show-all must reveal the same shared paint/pick occurrence"
        );
        let mut editor = ConstraintEditor::default();
        editor.set_selection([SelectionItem::Curve(related_curve)]);
        let effects =
            editor.pointer_move(&scene, pointer(9, marker.x, marker.y, Modifiers::default()));
        let occurrence = SceneAnnotationOccurrence {
            item: horizontal,
            marker_index: Some(0),
        };
        assert_eq!(
            effects,
            vec![EditorEffect::HoverChanged(EditorHoverState {
                target: Some(EditorHoverTarget::Annotation(occurrence)),
                context_owner: None,
            })]
        );
        let effects =
            editor.pointer_down(&scene, pointer(9, marker.x, marker.y, Modifiers::default()));
        assert_eq!(editor.selection(), &[horizontal]);
        assert_eq!(
            effects,
            vec![EditorEffect::SelectionChanged(vec![horizontal])]
        );
        assert_eq!(editor.hover_state(), EditorHoverState::default());
        assert!(editor.pointer_leave().is_empty());
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one M76 transition fixture compares threshold, preview, cancellation, commit, pointer, camera, and reset invariants"
    )]
    fn m76_annotation_drag_threshold_cancel_commit_and_reset_are_presentation_only() {
        let mut document = SketchDocument::new(8.0).expect("document");
        let start = document.add_point("start", [0.0, 0.0]).expect("start");
        let end = document.add_point("end", [4.0, 0.0]).expect("end");
        let line = document
            .add_curve(
                "line",
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction: [1.0, 0.0],
                },
            )
            .expect("line");
        let constraint = document
            .add_constraint(
                "horizontal",
                DocumentConstraintDefinition::Horizontal {
                    line: CurveSpan::line(line),
                },
            )
            .expect("constraint");
        let scene = scene(&document);
        let annotation = scene
            .annotations
            .iter()
            .find(|annotation| annotation.item == SelectionItem::Constraint(constraint))
            .expect("annotation");
        let SceneAnnotationGeometry::Glyph { markers } = &annotation.geometry else {
            panic!("horizontal annotation must be a glyph")
        };
        let origin = markers[0].anchor;
        let mut editor = ConstraintEditor::default();
        editor.set_selection([SelectionItem::Curve(CurveSpan::line(line))]);
        editor.pointer_move(
            &scene,
            pointer(76, origin.x, origin.y, Modifiers::default()),
        );
        editor.pointer_down(
            &scene,
            pointer(76, origin.x, origin.y, Modifiers::default()),
        );
        assert_eq!(
            editor.active_pointer_gesture(),
            Some(ActivePointerGesture {
                pointer_id: 76,
                kind: ActivePointerGestureKind::Annotation,
            })
        );
        assert!(
            editor
                .pointer_move(
                    &scene,
                    pointer(76, origin.x + 2.9, origin.y, Modifiers::default()),
                )
                .is_empty()
        );
        assert!(editor.annotation_layout().entries().is_empty());
        editor.pointer_move(
            &scene,
            pointer(76, origin.x + 12.0, origin.y - 7.0, Modifiers::default()),
        );
        assert!(
            editor.annotation_layout().entries().is_empty(),
            "an in-flight preview must not enter the persistable cache",
        );
        let preview_layout = editor.annotation_layout_for_scene();
        assert_eq!(preview_layout.entries().len(), 1);
        let mut moved_scene = scene.clone();
        moved_scene.apply_annotation_layout(&preview_layout);
        let moved = match &moved_scene
            .annotations
            .iter()
            .find(|candidate| candidate.item == annotation.item)
            .expect("moved annotation")
            .geometry
        {
            SceneAnnotationGeometry::Glyph { markers } => markers[0].anchor,
            _ => unreachable!(),
        };
        assert_eq!(
            moved,
            ScreenPoint {
                x: origin.x + 12.0,
                y: origin.y - 7.0
            }
        );
        editor.cancel();
        assert!(editor.annotation_layout().entries().is_empty());
        assert!(editor.annotation_layout_for_scene().entries().is_empty());

        editor.set_selection([SelectionItem::Curve(CurveSpan::line(line))]);
        editor.pointer_move(
            &scene,
            pointer(78, origin.x, origin.y, Modifiers::default()),
        );
        editor.pointer_down(
            &scene,
            pointer(78, origin.x, origin.y, Modifiers::default()),
        );
        editor.pointer_move(
            &scene,
            pointer(78, origin.x + 10.0, origin.y, Modifiers::default()),
        );
        assert_eq!(editor.annotation_layout_for_scene().entries().len(), 1);
        editor.activate_tool(EditorTool::Line);
        assert!(editor.annotation_layout_for_scene().entries().is_empty());
        editor.activate_tool(EditorTool::Select);

        editor.set_selection([SelectionItem::Curve(CurveSpan::line(line))]);
        editor.pointer_move(
            &scene,
            pointer(79, origin.x, origin.y, Modifiers::default()),
        );
        editor.pointer_down(
            &scene,
            pointer(79, origin.x, origin.y, Modifiers::default()),
        );
        editor.pointer_move(
            &scene,
            pointer(79, origin.x, origin.y + 10.0, Modifiers::default()),
        );
        assert_eq!(editor.annotation_layout_for_scene().entries().len(), 1);
        editor.invalidate_draft_inference();
        assert!(editor.annotation_layout_for_scene().entries().is_empty());

        editor.set_selection([SelectionItem::Curve(CurveSpan::line(line))]);
        editor.pointer_move(
            &scene,
            pointer(77, origin.x, origin.y, Modifiers::default()),
        );
        editor.pointer_down(
            &scene,
            pointer(77, origin.x, origin.y, Modifiers::default()),
        );
        let released = pointer(77, origin.x + 8.0, origin.y + 6.0, Modifiers::default());
        editor.pointer_move(&scene, released);
        assert!(
            editor
                .pointer_up(&scene, scene.design_identity, released)
                .is_empty()
        );
        assert_eq!(editor.annotation_layout().entries().len(), 1);
        editor.set_selection([annotation.item]);
        assert!(editor.reset_selected_annotation_layout());
        assert!(editor.annotation_layout().entries().is_empty());

        editor.set_selection([SelectionItem::Curve(CurveSpan::line(line))]);
        editor.pointer_move(
            &scene,
            pointer(80, origin.x, origin.y, Modifiers::default()),
        );
        editor.pointer_down(
            &scene,
            pointer(80, origin.x, origin.y, Modifiers::default()),
        );
        editor.pointer_move(
            &scene,
            pointer(81, origin.x + 15.0, origin.y, Modifiers::default()),
        );
        assert!(editor.annotation_layout_for_scene().entries().is_empty());
        assert_eq!(
            editor
                .active_pointer_gesture()
                .map(|gesture| gesture.pointer_id),
            Some(80)
        );
        editor.pointer_move(
            &scene,
            pointer(80, origin.x + 15.0, origin.y, Modifiers::default()),
        );
        assert_eq!(editor.annotation_layout_for_scene().entries().len(), 1);
        let mut changed_camera = scene.clone();
        changed_camera.viewport = Viewport::new(
            scene.viewport.screen_size,
            [1.0, -1.0],
            scene.viewport.pixels_per_model_unit,
        )
        .expect("changed camera");
        editor.pointer_move(
            &changed_camera,
            pointer(80, origin.x + 16.0, origin.y, Modifiers::default()),
        );
        assert!(editor.active_pointer_gesture().is_none());
        assert!(editor.annotation_layout().entries().is_empty());

        editor.pointer_move(
            &scene,
            pointer(82, origin.x, origin.y, Modifiers::default()),
        );
        editor.pointer_down(
            &scene,
            pointer(82, origin.x, origin.y, Modifiers::default()),
        );
        let moved = pointer(82, origin.x + 9.0, origin.y, Modifiers::default());
        editor.pointer_move(&scene, moved);
        editor.pointer_up(
            &scene,
            scene.design_identity,
            pointer(83, moved.position.x, moved.position.y, Modifiers::default()),
        );
        assert_eq!(
            editor
                .active_pointer_gesture()
                .map(|gesture| gesture.pointer_id),
            Some(82)
        );
        let mut stale_scene = scene.clone();
        stale_scene.accepted_revision = stale_scene.accepted_revision.wrapping_add(1);
        editor.pointer_up(&stale_scene, scene.design_identity, moved);
        assert!(editor.active_pointer_gesture().is_none());
        assert!(editor.annotation_layout().entries().is_empty());

        let second_item = SelectionItem::Constraint(DocumentConstraintId(
            geosolve_sketch::PersistentId::from_u128(0x76),
        ));
        let first_key = annotation.layout_key(scene.accepted_document.id(), Some(0));
        let second_key = AnnotationLayoutKey {
            item: second_item,
            ..first_key
        };
        editor.restore_annotation_layout(AnnotationLayoutState::from_entries([
            AnnotationLayoutEntry {
                key: first_key,
                placement: AnnotationPlacement::Free {
                    offset_pixels: [8.0, 6.0],
                },
            },
            AnnotationLayoutEntry {
                key: second_key,
                placement: AnnotationPlacement::Free {
                    offset_pixels: [-4.0, 9.0],
                },
            },
        ]));
        editor.set_selection([annotation.item, second_item]);
        assert!(editor.reset_selected_annotation_layout());
        assert!(editor.annotation_layout().entries().is_empty());
    }

    #[test]
    fn m76_multi_marker_layout_moves_only_the_exact_glyph_occurrence() {
        let (mut document, lines, _) = line_document();
        let parallel = document
            .add_constraint(
                "parallel occurrence layout",
                DocumentConstraintDefinition::Parallel {
                    first: lines[0],
                    second: lines[1],
                },
            )
            .expect("parallel constraint");
        let scene = scene(&document);
        let annotation = scene
            .annotations
            .iter()
            .find(|annotation| annotation.item == SelectionItem::Constraint(parallel))
            .expect("parallel annotation");
        let SceneAnnotationGeometry::Glyph { markers } = &annotation.geometry else {
            panic!("parallel annotation must use glyph occurrences")
        };
        assert_eq!(markers.len(), 2);
        let original = [markers[0].anchor, markers[1].anchor];
        let mut editor = ConstraintEditor::default();
        editor.set_selection([SelectionItem::Curve(lines[1])]);
        editor.pointer_move(
            &scene,
            pointer(76, original[1].x, original[1].y, Modifiers::default()),
        );
        editor.pointer_down(
            &scene,
            pointer(76, original[1].x, original[1].y, Modifiers::default()),
        );
        let released = pointer(
            76,
            original[1].x + 11.0,
            original[1].y - 5.0,
            Modifiers::default(),
        );
        editor.pointer_move(&scene, released);
        editor.pointer_up(&scene, scene.design_identity, released);
        let entries = editor.annotation_layout().entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key.marker_index, Some(1));

        let mut moved_scene = scene.clone();
        moved_scene.apply_annotation_layout(editor.annotation_layout());
        let moved_annotation = moved_scene
            .annotations
            .iter()
            .find(|annotation| annotation.item == SelectionItem::Constraint(parallel))
            .expect("moved parallel annotation");
        let SceneAnnotationGeometry::Glyph { markers } = &moved_annotation.geometry else {
            unreachable!()
        };
        assert_eq!(markers[0].anchor, original[0]);
        assert_eq!(
            markers[1].anchor,
            ScreenPoint {
                x: original[1].x + 11.0,
                y: original[1].y - 5.0,
            },
        );
    }

    #[test]
    fn m76_manual_glyph_placement_is_reserved_by_recomputed_automatic_neighbors() {
        let mut document = SketchDocument::new(8.0).expect("document");
        let start = document.add_point("start", [0.0, 0.0]).expect("start");
        let end = document.add_point("end", [4.0, 0.0]).expect("end");
        let line = CurveSpan::line(
            document
                .add_curve(
                    "line",
                    CurveDefinition::Line {
                        start,
                        end,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .expect("line"),
        );
        let constraints = ["first horizontal", "second horizontal"].map(|label| {
            document
                .add_constraint(label, DocumentConstraintDefinition::Horizontal { line })
                .expect("horizontal constraint")
        });
        let mut scene = scene(&document);
        let anchors = constraints.map(|id| {
            scene
                .annotations
                .iter()
                .find(|annotation| annotation.item == SelectionItem::Constraint(id))
                .and_then(|annotation| match &annotation.geometry {
                    SceneAnnotationGeometry::Glyph { markers } => {
                        markers.first().map(|marker| marker.anchor)
                    }
                    SceneAnnotationGeometry::RightAngle { .. }
                    | SceneAnnotationGeometry::LinearDimension { .. }
                    | SceneAnnotationGeometry::RadialDimension { .. }
                    | SceneAnnotationGeometry::AngularDimension { .. }
                    | SceneAnnotationGeometry::Label { .. } => None,
                })
                .expect("glyph anchor")
        });
        let first_annotation = scene
            .annotations
            .iter()
            .find(|annotation| annotation.item == SelectionItem::Constraint(constraints[0]))
            .expect("first annotation");
        let desired = anchors[1];
        let layout = AnnotationLayoutState::from_entries([AnnotationLayoutEntry {
            key: first_annotation.layout_key(scene.accepted_document.id(), Some(0)),
            placement: AnnotationPlacement::Free {
                offset_pixels: [desired.x - anchors[0].x, desired.y - anchors[0].y],
            },
        }]);
        scene.apply_annotation_layout(&layout);
        let moved = constraints.map(|id| {
            scene
                .annotations
                .iter()
                .find(|annotation| annotation.item == SelectionItem::Constraint(id))
                .and_then(|annotation| match &annotation.geometry {
                    SceneAnnotationGeometry::Glyph { markers } => {
                        markers.first().map(|marker| marker.anchor)
                    }
                    SceneAnnotationGeometry::RightAngle { .. }
                    | SceneAnnotationGeometry::LinearDimension { .. }
                    | SceneAnnotationGeometry::RadialDimension { .. }
                    | SceneAnnotationGeometry::AngularDimension { .. }
                    | SceneAnnotationGeometry::Label { .. } => None,
                })
                .expect("moved glyph anchor")
        });
        assert_eq!(moved[0], desired, "manual occurrence must never auto-move");
        assert!(
            moved[1].distance(desired) >= 22.0 - 1.0e-9,
            "automatic sibling must reserve the explicit manual placement",
        );
    }

    #[test]
    fn m76_annotation_commit_changes_no_solve_revision_or_sketch_history() {
        let mut document = SketchDocument::new(8.0).expect("document");
        let start = document.add_point("start", [0.0, 0.0]).expect("start");
        let end = document.add_point("end", [4.0, 0.0]).expect("end");
        let line = document
            .add_curve(
                "line",
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction: [1.0, 0.0],
                },
            )
            .expect("line");
        let constraint = document
            .add_constraint(
                "horizontal",
                DocumentConstraintDefinition::Horizontal {
                    line: CurveSpan::line(line),
                },
            )
            .expect("constraint");
        let session = geosolve_sketch::RetainedSketchDocumentSession::new(
            document,
            geosolve_sketch::DocumentSolveRequest::default(),
            geosolve_sketch::SolverConfig::default(),
        )
        .expect("session");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let accepted = coordinator
            .session()
            .accepted_state_for_current_input()
            .expect("accepted");
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            coordinator.session().design_identity(),
            accepted.document(),
            coordinator.session().design_document(),
            Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport"),
            0.5,
        )
        .expect("scene");
        let marker = scene
            .annotations
            .iter()
            .find(|annotation| annotation.item == SelectionItem::Constraint(constraint))
            .and_then(|annotation| match &annotation.geometry {
                SceneAnnotationGeometry::Glyph { markers } => markers.first(),
                _ => None,
            })
            .expect("horizontal marker")
            .anchor;
        let before_history = (coordinator.history_len(), coordinator.history_cursor());
        let before_design = coordinator.session().design_identity();
        let before_accepted = coordinator
            .session()
            .accepted_state_for_current_input()
            .expect("accepted before")
            .identity();
        let before_checkpoint = coordinator
            .persistence_checkpoint()
            .expect("checkpoint before");

        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Curve(CurveSpan::line(line))]);
        coordinator.editor_mut().pointer_move(
            &scene,
            pointer(76, marker.x, marker.y, Modifiers::default()),
        );
        coordinator.editor_mut().pointer_down(
            &scene,
            pointer(76, marker.x, marker.y, Modifiers::default()),
        );
        let released = pointer(76, marker.x + 12.0, marker.y + 8.0, Modifiers::default());
        coordinator.editor_mut().pointer_move(&scene, released);
        coordinator
            .editor_mut()
            .pointer_up(&scene, before_design, released);

        assert_eq!(coordinator.editor().annotation_layout().entries().len(), 1);
        assert_eq!(
            (coordinator.history_len(), coordinator.history_cursor()),
            before_history,
        );
        assert_eq!(coordinator.session().design_identity(), before_design);
        assert_eq!(
            coordinator
                .session()
                .accepted_state_for_current_input()
                .expect("accepted after")
                .identity(),
            before_accepted,
        );
        let after_checkpoint = coordinator
            .persistence_checkpoint()
            .expect("checkpoint after");
        assert_eq!(after_checkpoint.revisions(), before_checkpoint.revisions());
        assert!(!coordinator.can_undo());
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one end-to-end pointer path makes context retention and occurrence transitions explicit"
    )]
    fn m63_hover_keeps_geometry_context_while_targeting_exact_icon_occurrences() {
        let mut document = SketchDocument::new(8.0).expect("document");
        let start = document.add_point("start", [-2.0, 0.0]).expect("start");
        let end = document.add_point("end", [2.0, 0.0]).expect("end");
        let line = document
            .add_curve(
                "hover bridge",
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction: [1.0, 0.0],
                },
            )
            .expect("line");
        for label in [
            "horizontal",
            "first duplicate",
            "second duplicate",
            "third duplicate",
            "fourth duplicate",
            "fifth duplicate",
        ] {
            document
                .add_constraint(
                    label,
                    DocumentConstraintDefinition::Horizontal {
                        line: CurveSpan::line(line),
                    },
                )
                .expect("horizontal constraint");
        }
        let scene = scene(&document);
        let related_curve = CurveSpan::line(line);
        let related_annotations = scene
            .annotations
            .iter()
            .filter(|annotation| {
                annotation
                    .operands
                    .contains(&SelectionItem::Curve(related_curve))
                    && matches!(annotation.geometry, SceneAnnotationGeometry::Glyph { .. })
            })
            .collect::<Vec<_>>();
        assert_eq!(related_annotations.len(), 6);
        let mut displaced = related_annotations
            .iter()
            .filter_map(|annotation| match &annotation.geometry {
                SceneAnnotationGeometry::Glyph { markers } => markers
                    .first()
                    .filter(|marker| marker.leader_from.is_some())
                    .map(|marker| (*annotation, *marker)),
                _ => None,
            })
            .collect::<Vec<_>>();
        displaced.sort_by(|first, second| {
            let first_origin = first.1.leader_from.expect("displaced marker");
            let second_origin = second.1.leader_from.expect("displaced marker");
            (second.1.anchor.y - second_origin.y)
                .abs()
                .total_cmp(&(first.1.anchor.y - first_origin.y).abs())
        });
        assert!(displaced.len() >= 2);
        let (first_annotation, first_marker) = displaced[0];
        let (second_annotation, second_marker) = displaced[1];
        let curve_geometry = scene
            .curves
            .iter()
            .find(|curve| curve.span == related_curve)
            .expect("related curve geometry");
        let [curve_start, curve_end] = [
            curve_geometry.screen_polyline[0],
            *curve_geometry.screen_polyline.last().expect("curve end"),
        ];
        let context_origin = ScreenPoint {
            x: curve_start.x * 0.75 + curve_end.x * 0.25,
            y: curve_start.y * 0.75 + curve_end.y * 0.25,
        };
        let bridge = (1..20)
            .map(|step| {
                let ratio = f64::from(step) / 20.0;
                ScreenPoint {
                    x: (first_marker.anchor.x - context_origin.x).mul_add(ratio, context_origin.x),
                    y: (first_marker.anchor.y - context_origin.y).mul_add(ratio, context_origin.y),
                }
            })
            .find(|position| {
                scene
                    .hit_test_with_policy(
                        *position,
                        PickTolerance::default(),
                        native_geometry_policy(),
                    )
                    .is_none()
                    && scene
                        .annotation_occurrence_hit_test(
                            *position,
                            PickTolerance::default(),
                            &[],
                            Some(SelectionItem::Curve(related_curve)),
                            &[],
                        )
                        .is_none()
                    && scene.contextual_annotation_transit(
                        *position,
                        PickTolerance::default(),
                        &[],
                        SelectionItem::Curve(related_curve),
                        context_origin,
                        &[],
                    )
            })
            .expect("bounded corridor must contain non-geometry transit");
        let context_state = |target| EditorHoverState {
            target,
            context_owner: Some(SelectionItem::Curve(related_curve)),
        };
        let mut hover_editor = ConstraintEditor::default();
        assert_eq!(
            hover_editor.pointer_move(
                &scene,
                pointer(10, context_origin.x, context_origin.y, Modifiers::default()),
            ),
            vec![EditorEffect::HoverChanged(context_state(Some(
                EditorHoverTarget::Geometry(SelectionItem::Curve(related_curve))
            )))]
        );
        assert!(
            related_annotations
                .iter()
                .all(|annotation| annotation.is_visible(
                    &[],
                    Some(SelectionItem::Curve(related_curve)),
                    &[]
                )),
            "geometry context must reveal every directly related sibling"
        );
        assert!(
            !first_annotation.hit_test(bridge, PickTolerance::default().annotation_pixels),
            "corridor transit must not count as icon proximity"
        );
        assert!(
            scene
                .hit_test_with_policy(bridge, PickTolerance::default(), native_geometry_policy(),)
                .is_none(),
            "off-leader bridge point must be outside geometry"
        );
        assert_eq!(
            hover_editor.pointer_move(
                &scene,
                pointer(10, bridge.x, bridge.y, Modifiers::default()),
            ),
            vec![EditorEffect::HoverChanged(context_state(None))]
        );
        assert!(
            hover_editor
                .pointer_down(
                    &scene,
                    pointer(10, bridge.x, bridge.y, Modifiers::default()),
                )
                .is_empty(),
            "a context-only corridor must not manufacture a click owner"
        );
        assert!(hover_editor.selection().is_empty());

        let first_occurrence = SceneAnnotationOccurrence {
            item: first_annotation.item,
            marker_index: Some(0),
        };
        assert_eq!(
            hover_editor.pointer_move(
                &scene,
                pointer(
                    10,
                    first_marker.anchor.x,
                    first_marker.anchor.y,
                    Modifiers::default(),
                ),
            ),
            vec![EditorEffect::HoverChanged(context_state(Some(
                EditorHoverTarget::Annotation(first_occurrence)
            )))]
        );
        assert_eq!(
            hover_editor.hover_state(),
            context_state(Some(EditorHoverTarget::Annotation(first_occurrence)))
        );
        let effects = hover_editor.pointer_down(
            &scene,
            pointer(
                10,
                first_marker.anchor.x,
                first_marker.anchor.y,
                Modifiers::default(),
            ),
        );
        assert_eq!(hover_editor.selection(), &[first_annotation.item]);
        assert_eq!(
            effects,
            vec![EditorEffect::SelectionChanged(vec![first_annotation.item])]
        );
        assert_eq!(
            hover_editor.hover_state(),
            EditorHoverState::default(),
            "selection-driven visibility must revoke the old annotation navigation context",
        );
        assert!(
            hover_editor
                .pointer_up(
                    &scene,
                    scene.design_identity,
                    pointer(
                        10,
                        first_marker.anchor.x,
                        first_marker.anchor.y,
                        Modifiers::default(),
                    ),
                )
                .is_empty(),
            "an annotation click below the drag threshold must only end capture",
        );
        assert_eq!(
            hover_editor.pointer_move(
                &scene,
                pointer(10, context_origin.x, context_origin.y, Modifiers::default()),
            ),
            vec![EditorEffect::HoverChanged(context_state(Some(
                EditorHoverTarget::Geometry(SelectionItem::Curve(related_curve))
            )))],
            "a fresh geometry sample must reacquire context after selection changes",
        );

        let second_occurrence = SceneAnnotationOccurrence {
            item: second_annotation.item,
            marker_index: Some(0),
        };
        let first_leader_origin = first_marker.leader_from.expect("displaced marker leader");
        let first_leader_sample = (4..=15)
            .map(|step| {
                let ratio = f64::from(step) / 20.0;
                ScreenPoint {
                    x: (first_marker.anchor.x - first_leader_origin.x)
                        .mul_add(ratio, first_leader_origin.x),
                    y: (first_marker.anchor.y - first_leader_origin.y)
                        .mul_add(ratio, first_leader_origin.y),
                }
            })
            .find(|position| {
                scene
                    .annotation_occurrence_hit_test(
                        *position,
                        PickTolerance::default(),
                        hover_editor.selection(),
                        Some(SelectionItem::Curve(related_curve)),
                        &[],
                    )
                    .is_some_and(|(occurrence, _)| occurrence == first_occurrence)
                    && position.distance(first_marker.anchor)
                        > SceneGlyphMarker::BOUND_RADIUS_PIXELS
            })
            .expect("the painted leader must expose an unambiguous owning sample");
        assert_eq!(
            hover_editor.pointer_move(
                &scene,
                pointer(
                    10,
                    first_leader_sample.x,
                    first_leader_sample.y,
                    Modifiers::default(),
                ),
            ),
            vec![EditorEffect::HoverChanged(context_state(Some(
                EditorHoverTarget::Annotation(first_occurrence)
            )))],
            "a painted glyph leader must retain the exact owning annotation occurrence",
        );
        assert_eq!(
            hover_editor.pointer_move(
                &scene,
                pointer(
                    10,
                    second_marker.anchor.x,
                    second_marker.anchor.y,
                    Modifiers::default(),
                ),
            ),
            vec![EditorEffect::HoverChanged(context_state(Some(
                EditorHoverTarget::Annotation(second_occurrence)
            )))],
        );
        assert_eq!(
            hover_editor.hover_state(),
            context_state(Some(EditorHoverTarget::Annotation(second_occurrence)))
        );
        assert_eq!(
            hover_editor.pointer_move(&scene, pointer(10, 10.0, 10.0, Modifiers::default())),
            vec![EditorEffect::HoverChanged(EditorHoverState::default())]
        );
    }

    #[test]
    fn m63_multi_marker_constraint_hover_is_occurrence_specific() {
        let (mut document, lines, _) = line_document();
        let parallel = document
            .add_constraint(
                "parallel pair",
                DocumentConstraintDefinition::Parallel {
                    first: lines[0],
                    second: lines[1],
                },
            )
            .expect("parallel constraint");
        let scene = scene(&document);
        let annotation = scene
            .annotations
            .iter()
            .find(|annotation| annotation.item == SelectionItem::Constraint(parallel))
            .expect("parallel annotation");
        let SceneAnnotationGeometry::Glyph { markers } = &annotation.geometry else {
            panic!("parallel relation must render glyph markers");
        };
        assert_eq!(markers.len(), 2);
        for (line, marker) in lines.iter().zip(markers) {
            let curve = scene
                .curves
                .iter()
                .find(|curve| curve.span == *line)
                .expect("parallel line geometry");
            let start = *curve.screen_polyline.first().expect("line start");
            let end = *curve.screen_polyline.last().expect("line end");
            let origin = ScreenPoint {
                x: (start.x + end.x) * 0.5,
                y: (start.y + end.y) * 0.5,
            };
            assert_eq!(
                marker.anchor,
                ScreenPoint {
                    x: origin.x + 24.0,
                    y: origin.y - 24.0,
                }
            );
            assert_ne!(marker.anchor, start);
            assert_ne!(marker.anchor, end);
            assert_eq!(marker.leader_from, Some(origin));
        }

        let mut editor = ConstraintEditor::default();
        editor.set_selection([SelectionItem::Curve(lines[0])]);
        for (marker_index, marker) in markers.iter().enumerate() {
            let expected = EditorHoverState {
                target: Some(EditorHoverTarget::Annotation(SceneAnnotationOccurrence {
                    item: annotation.item,
                    marker_index: Some(marker_index),
                })),
                context_owner: None,
            };
            assert_eq!(
                editor.pointer_move(
                    &scene,
                    pointer(11, marker.anchor.x, marker.anchor.y, Modifiers::default(),),
                ),
                vec![EditorEffect::HoverChanged(expected)]
            );
            assert_eq!(editor.hover_state(), expected);
        }
    }

    #[test]
    fn m63_perpendicular_relation_uses_selectable_square_between_lines() {
        let mut document = SketchDocument::new(8.0).expect("document");
        let vertex = document.add_point("vertex", [0.0, 0.0]).expect("vertex");
        let right = document.add_point("right", [4.0, 0.0]).expect("right");
        let up = document.add_point("up", [0.0, 3.0]).expect("up");
        let horizontal = CurveSpan::line(
            document
                .add_curve(
                    "horizontal",
                    CurveDefinition::Line {
                        start: right,
                        end: vertex,
                        branch_direction: [-1.0, 0.0],
                    },
                )
                .expect("horizontal line"),
        );
        let vertical = CurveSpan::line(
            document
                .add_curve(
                    "vertical",
                    CurveDefinition::Line {
                        start: up,
                        end: vertex,
                        branch_direction: [0.0, -1.0],
                    },
                )
                .expect("vertical line"),
        );
        let perpendicular = document
            .add_constraint(
                "right angle",
                DocumentConstraintDefinition::Perpendicular {
                    first: horizontal,
                    second: vertical,
                },
            )
            .expect("perpendicular constraint");

        let scene = scene(&document);
        let annotation = scene
            .annotations
            .iter()
            .find(|annotation| annotation.item == SelectionItem::Constraint(perpendicular))
            .expect("perpendicular annotation");
        let SceneAnnotationGeometry::RightAngle {
            vertex,
            first_arm,
            corner,
            second_arm,
        } = &annotation.geometry
        else {
            panic!("perpendicular relation must render a right-angle square");
        };
        assert_eq!(*vertex, ScreenPoint { x: 500.0, y: 350.0 });
        assert_eq!(*first_arm, ScreenPoint { x: 512.0, y: 350.0 });
        assert_eq!(*corner, ScreenPoint { x: 512.0, y: 338.0 });
        assert_eq!(*second_arm, ScreenPoint { x: 500.0, y: 338.0 });
        assert!(annotation.hit_test(*corner, 0.0));
        assert_eq!(annotation.context_anchors(), vec![*corner]);
        assert_eq!(
            annotation.operands,
            vec![
                SelectionItem::Curve(horizontal),
                SelectionItem::Curve(vertical)
            ]
        );

        document
            .set_element_user_suppressed(
                geosolve_sketch::DocumentElementId::Constraint(perpendicular),
                true,
            )
            .expect("suppress perpendicular relation");
        let suppressed_scene = crate::tests::scene(&document);
        let suppressed = suppressed_scene
            .annotations
            .iter()
            .find(|annotation| annotation.item == SelectionItem::Constraint(perpendicular))
            .expect("suppressed annotation");
        assert!(suppressed.suppressed);
        assert!(matches!(
            suppressed.geometry,
            SceneAnnotationGeometry::Glyph { .. }
        ));
    }

    #[test]
    fn m76_symmetry_glyph_rotates_with_its_oblique_local_axis() {
        let mut document = SketchDocument::new(8.0).expect("document");
        let axis_start = document
            .add_point("axis start", [-2.0, -2.0])
            .expect("axis start");
        let axis_end = document
            .add_point("axis end", [2.0, 2.0])
            .expect("axis end");
        let first = document
            .add_point("first symmetric point", [2.0, 0.0])
            .expect("first symmetric point");
        let second = document
            .add_point("second symmetric point", [0.0, 2.0])
            .expect("second symmetric point");
        let axis = CurveSpan::line(
            document
                .add_curve(
                    "oblique symmetry axis",
                    CurveDefinition::Line {
                        start: axis_start,
                        end: axis_end,
                        branch_direction: [std::f64::consts::FRAC_1_SQRT_2; 2],
                    },
                )
                .expect("axis"),
        );
        let constraint = document
            .add_constraint(
                "oblique symmetry",
                DocumentConstraintDefinition::SymmetricAboutLine {
                    first,
                    second,
                    line: axis,
                },
            )
            .expect("symmetry constraint");
        let scene = scene(&document);
        let marker = scene
            .annotations
            .iter()
            .find(|annotation| annotation.item == SelectionItem::Constraint(constraint))
            .and_then(|annotation| match &annotation.geometry {
                SceneAnnotationGeometry::Glyph { markers } => markers.first(),
                SceneAnnotationGeometry::RightAngle { .. }
                | SceneAnnotationGeometry::LinearDimension { .. }
                | SceneAnnotationGeometry::RadialDimension { .. }
                | SceneAnnotationGeometry::AngularDimension { .. }
                | SceneAnnotationGeometry::Label { .. } => None,
            })
            .expect("symmetry marker");
        let curve = scene
            .curves
            .iter()
            .find(|curve| curve.span == axis)
            .expect("axis scene curve");
        let start = curve.screen_polyline.first().expect("axis start");
        let end = curve.screen_polyline.last().expect("axis end");
        let length = start.distance(*end);
        let axis_direction = [(end.x - start.x) / length, (end.y - start.y) / length];
        let icon_axis = [
            -marker.rotation_radians.sin(),
            marker.rotation_radians.cos(),
        ];
        assert!(
            (axis_direction[0].mul_add(icon_axis[0], axis_direction[1] * icon_axis[1])).abs()
                > 1.0 - 1.0e-12,
            "the symmetry mark's local axis must follow the accepted oblique support",
        );
    }

    #[test]
    fn m63_radius_dimension_uses_a_stable_semantic_circle_branch() {
        let mut document = SketchDocument::new(8.0).expect("document");
        let center = document.add_point("center", [0.0, 0.0]).expect("center");
        let radius = document
            .add_scalar("radius", 2.0, ScalarUnit::Length, ScalarDomain::Positive)
            .expect("radius");
        let circle = document
            .add_curve("circle", CurveDefinition::Circle { center, radius })
            .expect("circle");
        let target = document
            .add_scalar(
                "radius target",
                2.0,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .expect("target");
        let dimension = document
            .add_dimension(
                "radius dimension",
                geosolve_sketch::DocumentDimensionDefinition::Radius {
                    curve: circle,
                    target,
                },
                DocumentDimensionMode::Reference,
            )
            .expect("dimension");

        let radial_geometry = |document: &SketchDocument| {
            let scene = scene(document);
            let annotation = scene
                .annotations
                .iter()
                .find(|annotation| annotation.item == SelectionItem::Dimension(dimension))
                .expect("radius annotation");
            match annotation.geometry {
                SceneAnnotationGeometry::RadialDimension { center, edge, .. } => (center, edge),
                _ => panic!("radius dimension must be radial"),
            }
        };

        let (first_center, first_edge) = radial_geometry(&document);
        assert_eq!(first_center, ScreenPoint { x: 500.0, y: 350.0 });
        assert_eq!(first_edge, ScreenPoint { x: 600.0, y: 350.0 });

        document
            .set_scalar_value(radius, 2.0 + 1.0e-10)
            .expect("nearby accepted radius");
        let (second_center, second_edge) = radial_geometry(&document);
        assert_eq!(second_center, first_center);
        assert!(second_edge.x > first_edge.x);
        assert!((second_edge.y - first_edge.y).abs() <= f64::EPSILON);
    }

    #[test]
    fn m76_reference_annotation_never_presents_its_dormant_target_as_measurement() {
        let mut document = SketchDocument::new(8.0).expect("document");
        let first = document.add_point("first", [0.0, 0.0]).expect("first");
        let second = document.add_point("second", [3.0, 4.0]).expect("second");
        let dormant_target = document
            .add_scalar(
                "dormant reference target",
                123.0,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .expect("target");
        let dimension = document
            .add_dimension(
                "actual endpoint distance",
                geosolve_sketch::DocumentDimensionDefinition::PointDistance {
                    first,
                    second,
                    target: dormant_target,
                },
                DocumentDimensionMode::Reference,
            )
            .expect("reference dimension");
        let session = RetainedSketchDocumentSession::new(
            document,
            geosolve_sketch::DocumentSolveRequest::default(),
            geosolve_sketch::SolverConfig::default(),
        )
        .expect("session");
        let accepted = session.accepted_state().expect("accepted");
        let mut scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport"),
            0.5,
        )
        .expect("scene");
        let annotation = scene
            .annotations
            .iter()
            .find(|annotation| annotation.item == SelectionItem::Dimension(dimension))
            .expect("reference annotation");
        assert_eq!(annotation.visible_text, None);
        assert!(annotation.accessible_label.contains("value unavailable"));
        assert!(!annotation.accessible_label.contains("123"));

        let mut divergent_document = accepted.document().clone();
        divergent_document
            .set_point_position(second, [0.0, 10.0])
            .expect("divergent second point");
        let divergent_session = RetainedSketchDocumentSession::new(
            divergent_document,
            geosolve_sketch::DocumentSolveRequest::default(),
            geosolve_sketch::SolverConfig::default(),
        )
        .expect("divergent same-document session");
        let divergent_accepted = divergent_session
            .accepted_state()
            .expect("divergent accepted");
        assert_eq!(divergent_accepted.document().id(), accepted.document().id());
        assert_ne!(divergent_accepted.document(), accepted.document());
        assert!(!scene.update_annotation_values(divergent_accepted));

        assert!(scene.update_annotation_values(accepted));
        let annotation = scene
            .annotations
            .iter()
            .find(|annotation| annotation.item == SelectionItem::Dimension(dimension))
            .expect("resolved reference annotation");
        assert_eq!(annotation.visible_text.as_deref(), Some("(5)"));
        assert!(annotation.accessible_label.contains("5 model units"));
        assert!(!annotation.accessible_label.contains("123"));
    }

    #[test]
    fn m63_rotating_square_annotations_have_non_overlapping_final_anchors() {
        let fixture = geosolve_sketch::alpha_scenario(
            geosolve_sketch::AlphaScenarioKind::MotionRotatingSquare,
            1.0,
        )
        .expect("rotating square");
        let scene = scene(&fixture.document);
        let right_angle_count = scene
            .annotations
            .iter()
            .filter(|annotation| {
                matches!(
                    &annotation.geometry,
                    SceneAnnotationGeometry::RightAngle { .. }
                )
            })
            .count();
        assert!(
            right_angle_count >= 1,
            "crowded fixture must exercise geometric right-angle presentation"
        );
        let anchors = scene
            .annotations
            .iter()
            .flat_map(|annotation| match &annotation.geometry {
                SceneAnnotationGeometry::Glyph { markers } => markers
                    .iter()
                    .map(|marker| marker.anchor)
                    .collect::<Vec<_>>(),
                SceneAnnotationGeometry::RightAngle { corner, .. } => vec![*corner],
                _ => Vec::new(),
            })
            .collect::<Vec<_>>();
        assert!(
            anchors.len() >= 8,
            "crowded fixture must retain representative density"
        );
        for (index, first) in anchors.iter().enumerate() {
            for second in &anchors[index + 1..] {
                assert!(
                    first.distance(*second) >= 22.0 - 1.0e-9,
                    "annotation anchors overlap at {first:?} and {second:?}"
                );
            }
        }
        assert!(
            scene.annotations.iter().any(|annotation| matches!(
                &annotation.geometry,
                SceneAnnotationGeometry::Glyph { markers }
                    if markers.iter().any(|marker| marker.leader_from.is_some())
            )),
            "crowded fixture must exercise displaced leaders"
        );
    }

    #[test]
    fn m63_catalog_projects_every_constraint_in_representative_public_scenarios() {
        for kind in [
            geosolve_sketch::AlphaScenarioKind::Corpus,
            geosolve_sketch::AlphaScenarioKind::NurbsDifferential,
            geosolve_sketch::AlphaScenarioKind::M27ReferenceFillet,
            geosolve_sketch::AlphaScenarioKind::M28TrimmedFillet,
            geosolve_sketch::AlphaScenarioKind::SupportingOffset,
            geosolve_sketch::AlphaScenarioKind::ExactTranslatedOffset,
        ] {
            let fixture = geosolve_sketch::alpha_scenario(kind, 1.0).expect("alpha scenario");
            let scene = scene(&fixture.document);
            let expected = fixture.document.constraints().iter().count();
            let actual = scene
                .annotations
                .iter()
                .filter(|annotation| matches!(annotation.kind, SceneAnnotationKind::Constraint(_)))
                .count();
            assert_eq!(
                actual, expected,
                "{kind:?} must project every active constraint"
            );
            let expected_dimensions = fixture.document.dimensions().iter().count();
            let actual_dimensions = scene
                .annotations
                .iter()
                .filter(|annotation| !matches!(annotation.kind, SceneAnnotationKind::Constraint(_)))
                .count();
            assert_eq!(
                actual_dimensions, expected_dimensions,
                "{kind:?} must project every active dimension"
            );
        }

        let mut directed =
            geosolve_sketch::alpha_scenario(geosolve_sketch::AlphaScenarioKind::DirectedAngle, 1.0)
                .expect("directed angle");
        let geosolve_sketch::AlphaScenarioIds::DirectedAngle(ids) = directed.ids else {
            panic!("directed angle ids");
        };
        directed
            .document
            .set_dimension_mode(ids.dimension, DocumentDimensionMode::Reference)
            .expect("reference angle");
        let scene = scene(&directed.document);
        let angle = scene
            .annotations
            .iter()
            .find(|annotation| annotation.item == SelectionItem::Dimension(ids.dimension))
            .expect("angle annotation");
        assert_eq!(angle.visibility, SceneAnnotationVisibility::Always);
        assert!(matches!(
            angle.geometry,
            SceneAnnotationGeometry::AngularDimension { .. }
        ));
    }

    #[test]
    fn line_is_selected_from_screen_space_without_dom_hit_targets() {
        let (document, spans, _) = line_document();
        let scene = scene(&document);
        for offset in [11.999, 12.0] {
            let hit = scene
                .hit_test_with_policy(
                    ScreenPoint {
                        x: 500.0,
                        y: 300.0 + offset,
                    },
                    PickTolerance::default(),
                    native_geometry_policy(),
                )
                .expect("line hit within the inclusive twelve-pixel radius");
            assert_eq!(hit.item, SelectionItem::Curve(spans[0]));
            assert!((hit.distance_pixels - offset).abs() < 1.0e-12);
            assert_eq!(hit.curve_parameter, Some(0.5));
        }
        assert!(
            scene
                .hit_test_with_policy(
                    ScreenPoint {
                        x: 500.0,
                        y: 312.001,
                    },
                    PickTolerance::default(),
                    native_geometry_policy(),
                )
                .is_none()
        );
    }

    #[test]
    fn point_has_priority_at_a_line_endpoint() {
        let (document, _, points) = line_document();
        let scene = scene(&document);
        let endpoint = scene.viewport.model_to_screen([-4.0, 1.0]);
        assert_eq!(
            scene.hit_test(endpoint, PickTolerance::default()),
            Some(Hit {
                item: SelectionItem::Point(points[0]),
                distance_pixels: 0.0,
                curve_parameter: None,
                geometry: Some(SceneGeometryHit::Point {
                    incidence: ScenePointRoleIncidence {
                        profile: true,
                        construction: false,
                    },
                }),
            })
        );
        let overlap = ScreenPoint {
            x: endpoint.x,
            y: endpoint.y + 7.5,
        };
        assert_eq!(
            scene.hit_test(overlap, PickTolerance::default()),
            Some(Hit {
                item: SelectionItem::Point(points[0]),
                distance_pixels: 7.5,
                curve_parameter: None,
                geometry: Some(SceneGeometryHit::Point {
                    incidence: ScenePointRoleIncidence {
                        profile: true,
                        construction: false,
                    },
                }),
            })
        );
        for offset in [7.999, 8.0, 8.001] {
            assert_eq!(
                scene
                    .hit_test(
                        ScreenPoint {
                            x: endpoint.x,
                            y: endpoint.y + offset,
                        },
                        PickTolerance::default(),
                    )
                    .is_some_and(|hit| hit.item == SelectionItem::Point(points[0])),
                offset <= 8.0,
            );
        }
    }

    #[test]
    fn native_authoring_candidates_preserve_point_priority_without_hiding_the_curve() {
        let (document, spans, points) = line_document();
        let scene = scene(&document);
        let endpoint = scene.viewport.model_to_screen([-4.0, 1.0]);
        let candidates = scene
            .native_authoring_hit_candidates(endpoint, PickTolerance::default(), 2)
            .expect("bounded candidates");

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].item, SelectionItem::Point(points[0]));
        assert_eq!(candidates[1].item, SelectionItem::Curve(spans[0]));
        assert_eq!(candidates[1].curve_parameter, Some(0.0));
        assert_eq!(
            scene.native_authoring_hit_test(endpoint, PickTolerance::default()),
            candidates.first().copied()
        );
        assert!(
            scene
                .native_authoring_hit_candidates(
                    ScreenPoint {
                        x: f64::NAN,
                        y: 0.0,
                    },
                    PickTolerance::default(),
                    2,
                )
                .expect("invalid positions produce no candidates")
                .is_empty()
        );
    }

    #[test]
    fn staged_role_and_kind_ranking_is_permutation_invariant() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let profile_start = document.add_point("ps", [-4.0, 0.0]).expect("point");
        let profile_end = document.add_point("pe", [4.0, 0.0]).expect("point");
        let construction_start = document.add_point("cs", [-4.0, 0.01]).expect("point");
        let construction_end = document.add_point("ce", [4.0, 0.01]).expect("point");
        let profile = document
            .add_curve_with_role(
                "profile",
                CurveDefinition::Line {
                    start: profile_start,
                    end: profile_end,
                    branch_direction: [1.0, 0.0],
                },
                GeometryRole::Profile,
            )
            .expect("curve");
        let construction = document
            .add_curve_with_role(
                "construction",
                CurveDefinition::Line {
                    start: construction_start,
                    end: construction_end,
                    branch_direction: [1.0, 0.0],
                },
                GeometryRole::Construction,
            )
            .expect("curve");
        let point = document.add_point("point", [0.0, 0.15]).expect("point");
        let original = scene(&document);
        let position = original.viewport.model_to_screen([0.0, 0.0]);
        let expected = [
            SelectionItem::Point(point),
            SelectionItem::Curve(CurveSpan::line(profile)),
            SelectionItem::Curve(CurveSpan::line(construction)),
        ];
        for (reverse_points, reverse_curves) in
            [(false, false), (true, false), (false, true), (true, true)]
        {
            let mut candidate = original.clone();
            if reverse_points {
                candidate.points.reverse();
            }
            if reverse_curves {
                candidate.curves.reverse();
            }
            assert_eq!(
                candidate
                    .hit_test_with_policy(
                        position,
                        PickTolerance::default(),
                        native_geometry_policy(),
                    )
                    .map(|hit| hit.item),
                Some(expected[0])
            );
            assert_eq!(
                candidate
                    .native_authoring_hit_candidates_with_policy(
                        position,
                        PickTolerance::default(),
                        3,
                        native_geometry_policy(),
                    )
                    .expect("bounded candidates")
                    .into_iter()
                    .map(|hit| hit.item)
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }

    #[test]
    fn native_authoring_candidate_collection_is_bounded_before_sorting() {
        let (document, _spans, _points) = line_document();
        let mut scene = scene(&document);
        let endpoint = scene.viewport.model_to_screen([-4.0, 1.0]);
        scene.curves.push(scene.curves[0].clone());

        let exact = scene
            .native_authoring_hit_candidates(endpoint, PickTolerance::default(), 2)
            .expect("a repeated fragment counts as one persistent curve");
        assert_eq!(exact.len(), 2);
        assert!(matches!(exact[0].item, SelectionItem::Point(_)));
        assert!(matches!(exact[1].item, SelectionItem::Curve(_)));

        assert_eq!(
            scene.native_authoring_hit_candidates(endpoint, PickTolerance::default(), 1),
            Err(NativeAuthoringHitError::CandidateLimitExceeded {
                maximum_candidates: 1,
            })
        );
        assert_eq!(
            scene.native_authoring_hit_candidates(endpoint, PickTolerance::default(), 0),
            Err(NativeAuthoringHitError::CandidateLimitExceeded {
                maximum_candidates: 0,
            })
        );
    }

    #[test]
    fn dense_parallel_line_picks_use_distance_then_persistent_identity() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let first_start = document.add_point("a", [-4.0, 0.16]).expect("point");
        let first_end = document.add_point("b", [4.0, 0.16]).expect("point");
        let second_start = document.add_point("c", [-4.0, -0.16]).expect("point");
        let second_end = document.add_point("d", [4.0, -0.16]).expect("point");
        let first = CurveSpan::line(
            document
                .add_curve(
                    "first",
                    CurveDefinition::Line {
                        start: first_start,
                        end: first_end,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .expect("curve"),
        );
        let second = CurveSpan::line(
            document
                .add_curve(
                    "second",
                    CurveDefinition::Line {
                        start: second_start,
                        end: second_end,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .expect("curve"),
        );
        let mut scene = scene(&document);
        scene.curves.reverse();

        let nearer = scene
            .hit_test(ScreenPoint { x: 500.0, y: 348.0 }, PickTolerance::default())
            .expect("both parallel lines are in range");
        assert_eq!(nearer.item, SelectionItem::Curve(first));
        assert!((nearer.distance_pixels - 6.0).abs() < 1.0e-12);

        let tie = scene
            .hit_test(ScreenPoint { x: 500.0, y: 350.0 }, PickTolerance::default())
            .expect("equidistant parallel lines are in range");
        assert_eq!(tie.item, SelectionItem::Curve(first.min(second)));
        assert!((tie.distance_pixels - 8.0).abs() < 1.0e-12);
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "separated, exact curve, and exact point preview barriers form one hit-policy regression"
    )]
    fn operation_hit_does_not_click_through_preview_created_geometry() {
        let (source, source_spans, source_points) = line_document();
        let mut preview = source.clone();
        let preview_start = preview
            .add_point("preview start", [-4.0, 1.1])
            .expect("point");
        let preview_end = preview.add_point("preview end", [4.0, 1.1]).expect("point");
        let preview_span = CurveSpan::line(
            preview
                .add_curve(
                    "preview line",
                    CurveDefinition::Line {
                        start: preview_start,
                        end: preview_end,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .expect("curve"),
        );
        let preview_scene = scene(&preview);
        let preview_position = ScreenPoint { x: 500.0, y: 295.0 };

        assert_eq!(
            preview_scene
                .hit_test(preview_position, PickTolerance::default())
                .map(|hit| hit.item),
            Some(SelectionItem::Curve(preview_span))
        );
        assert_eq!(
            preview_scene.hit_test_for_document(
                preview_position,
                PickTolerance::default(),
                &source,
            ),
            None,
            "the best preview-created hit must block rather than expose source geometry behind it"
        );

        let source_position = ScreenPoint { x: 500.0, y: 306.0 };
        assert_eq!(
            preview_scene
                .hit_test_for_document(source_position, PickTolerance::default(), &source)
                .map(|hit| hit.item),
            Some(SelectionItem::Curve(source_spans[0]))
        );

        let mut exact_overlap = source.clone();
        let overlap_start = exact_overlap
            .add_point("overlap start", [-4.0, 1.0])
            .expect("point");
        let overlap_end = exact_overlap
            .add_point("overlap end", [4.0, 1.0])
            .expect("point");
        let overlap_span = CurveSpan::line(
            exact_overlap
                .add_curve(
                    "foreground overlap",
                    CurveDefinition::Line {
                        start: overlap_start,
                        end: overlap_end,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .expect("curve"),
        );
        let overlap_scene = scene(&exact_overlap);
        let overlap_position = ScreenPoint { x: 500.0, y: 300.0 };
        assert_eq!(
            overlap_scene
                .hit_test(overlap_position, PickTolerance::default())
                .map(|hit| hit.item),
            Some(SelectionItem::Curve(source_spans[0])),
            "ordinary persistent-identity ties still prefer the older source span"
        );
        assert_ne!(overlap_span, source_spans[0]);
        assert_eq!(
            overlap_scene.hit_test_for_document(
                overlap_position,
                PickTolerance::default(),
                &source,
            ),
            None,
            "a preview-only curve at the exact same distance is the foreground click barrier"
        );
        let endpoint_position = overlap_scene
            .points
            .iter()
            .find(|point| point.id == source_points[0])
            .expect("source endpoint")
            .screen_position;
        assert!(matches!(
            overlap_scene.hit_test(endpoint_position, PickTolerance::default()),
            Some(Hit {
                item: SelectionItem::Point(point),
                ..
            }) if point == source_points[0]
        ));
        assert_eq!(
            overlap_scene.hit_test_for_document(
                endpoint_position,
                PickTolerance::default(),
                &source,
            ),
            None,
            "a preview-only point tied over a source point is also a foreground click barrier"
        );
    }

    #[test]
    fn extended_line_selection_resolves_and_applies_contextual_parallel_relation() {
        let (document, spans, _) = line_document();
        let scene = scene(&document);
        let mut editor = ConstraintEditor::default();
        editor.pointer_down(&scene, pointer(1, 500.0, 300.0, Modifiers::default()));
        editor.pointer_down(
            &scene,
            pointer(
                2,
                500.0,
                400.0,
                Modifiers {
                    shift: true,
                    ..Modifiers::default()
                },
            ),
        );
        assert_eq!(
            editor.selection(),
            &[
                SelectionItem::Curve(spans[0]),
                SelectionItem::Curve(spans[1])
            ]
        );

        let session = RetainedSketchDocumentSession::new(
            document,
            geosolve_sketch::DocumentSolveRequest::default(),
            geosolve_sketch::SolverConfig::default(),
        )
        .expect("coordinator session");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        coordinator
            .editor_mut()
            .set_selection(editor.selection().iter().copied());
        assert_eq!(
            coordinator.resolved_constraint(ConstraintIntent::Parallel),
            Some(ResolvedConstraintKind::ParallelLines)
        );
        let application = AuthoringState::default().activate(
            coordinator.session().design_document(),
            AuthoringTool::Constraint(ConstraintIntent::Parallel),
            &editor
                .selection()
                .iter()
                .copied()
                .map(AuthoringOperand::selected)
                .collect::<Vec<_>>(),
        );
        let AuthoringOutcome::Apply(application) = application else {
            panic!("contextual parallel application");
        };
        let AuthoringMutation::Constraint(outcome) = coordinator
            .apply_authoring(coordinator.session().design_identity(), &application)
            .expect("contextual parallel apply")
        else {
            panic!("constraint mutation");
        };
        assert!(matches!(
            coordinator
                .session()
                .design_document()
                .constraint(outcome.value)
                .expect("parallel definition")
                .definition,
            DocumentConstraintDefinition::Parallel { first, second }
                if first == spans[0] && second == spans[1]
        ));
    }

    #[test]
    fn m71_contextual_relation_availability_rejects_semantic_tautologies() {
        let (mut document, spans, points) = line_document();
        let assert_rejected =
            |document: &SketchDocument, selection: &[SelectionItem], intent: ConstraintIntent| {
                let operands = selection
                    .iter()
                    .copied()
                    .map(AuthoringOperand::selected)
                    .collect::<Vec<_>>();
                assert!(matches!(
                    AuthoringState::default().activate(
                        document,
                        AuthoringTool::Constraint(intent),
                        &operands,
                    ),
                    AuthoringOutcome::Warning(AuthoringWarning {
                        reason: DisabledReason::SameSemanticOperand,
                        ..
                    })
                ));
            };

        let repeated_point = [
            SelectionItem::Point(points[0]),
            SelectionItem::Point(points[0]),
        ];
        for intent in [ConstraintIntent::Horizontal, ConstraintIntent::Vertical] {
            assert_rejected(&document, &repeated_point, intent);
        }

        let first_radius = document
            .add_scalar(
                "first radius",
                1.0,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .expect("first radius");
        let second_radius = document
            .add_scalar(
                "second radius",
                2.0,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .expect("second radius");
        let first_circle = CurveSpan::line(
            document
                .add_curve(
                    "first circle",
                    CurveDefinition::Circle {
                        center: points[0],
                        radius: first_radius,
                    },
                )
                .expect("first circle"),
        );
        let second_circle = CurveSpan::line(
            document
                .add_curve(
                    "second circle",
                    CurveDefinition::Circle {
                        center: points[0],
                        radius: second_radius,
                    },
                )
                .expect("second circle"),
        );
        let shared_center = [
            SelectionItem::Curve(first_circle),
            SelectionItem::Curve(second_circle),
        ];
        assert_rejected(&document, &shared_center, ConstraintIntent::Concentric);

        let repeated_support = [
            SelectionItem::Curve(spans[0]),
            SelectionItem::Curve(spans[0]),
        ];
        assert_rejected(&document, &repeated_support, ConstraintIntent::Collinear);
    }

    #[test]
    fn m71_f002_contextual_relation_availability_rejects_missing_objects_and_invalid_spans() {
        let (mut document, _spans, points) = line_document();
        let foreign_point = DesignPointId(geosolve_sketch::PersistentId::from_u128(u128::MAX));
        let foreign_points = [
            SelectionItem::Point(points[0]),
            SelectionItem::Point(foreign_point),
        ];
        let session = RetainedSketchDocumentSession::new(
            document.clone(),
            geosolve_sketch::DocumentSolveRequest::default(),
            geosolve_sketch::SolverConfig::default(),
        )
        .expect("coordinator session");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        coordinator.editor_mut().set_selection(foreign_points);
        assert_eq!(
            coordinator
                .actions()
                .into_iter()
                .find(|availability| {
                    availability.action
                        == CoordinatorActionKind::Constraint(ConstraintIntent::Horizontal)
                })
                .expect("contextual horizontal availability")
                .state,
            ActionState::Disabled(DisabledReason::MissingObject)
        );

        let centers = [
            document.add_point("center a", [0.0, 0.0]).expect("point"),
            document.add_point("center b", [2.0, 0.0]).expect("point"),
        ];
        let radii = [1.0, 2.0].map(|value| {
            document
                .add_scalar("radius", value, ScalarUnit::Length, ScalarDomain::Positive)
                .expect("radius")
        });
        let curves = [0, 1].map(|index| {
            document
                .add_curve(
                    "circle",
                    CurveDefinition::Circle {
                        center: centers[index],
                        radius: radii[index],
                    },
                )
                .expect("circle")
        });
        let invalid_span = CurveSpan {
            curve: curves[0],
            segment: 71,
        };
        let session = RetainedSketchDocumentSession::new(
            document,
            geosolve_sketch::DocumentSolveRequest::default(),
            geosolve_sketch::SolverConfig::default(),
        )
        .expect("coordinator session");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        coordinator.editor_mut().set_selection([
            SelectionItem::Curve(invalid_span),
            SelectionItem::Curve(CurveSpan::line(curves[1])),
        ]);
        assert_eq!(
            coordinator
                .actions()
                .into_iter()
                .find(|availability| {
                    availability.action
                        == CoordinatorActionKind::Constraint(ConstraintIntent::Concentric)
                })
                .expect("contextual concentric availability")
                .state,
            ActionState::Disabled(DisabledReason::MissingObject)
        );
    }

    #[test]
    fn ordered_mixed_selection_replaces_extends_and_toggles_by_persistent_identity() {
        let (document, spans, points) = line_document();
        let mut editor = ConstraintEditor::default();
        let initial = [
            SelectionItem::Point(points[1]),
            SelectionItem::Curve(spans[0]),
            SelectionItem::Point(points[0]),
            SelectionItem::Curve(spans[0]),
        ];
        editor.set_selection(initial);
        assert_eq!(
            editor.selection(),
            &[
                SelectionItem::Point(points[1]),
                SelectionItem::Curve(spans[0]),
                SelectionItem::Point(points[0]),
            ]
        );

        editor.select_item(
            SelectionItem::Curve(spans[1]),
            Modifiers {
                shift: true,
                ..Modifiers::default()
            },
        );
        editor.select_item(
            SelectionItem::Point(points[2]),
            Modifiers {
                control: true,
                ..Modifiers::default()
            },
        );
        editor.select_item(
            SelectionItem::Point(points[3]),
            Modifiers {
                command: true,
                ..Modifiers::default()
            },
        );
        assert_eq!(
            editor.selection(),
            &[
                SelectionItem::Point(points[1]),
                SelectionItem::Curve(spans[0]),
                SelectionItem::Point(points[0]),
                SelectionItem::Curve(spans[1]),
                SelectionItem::Point(points[2]),
                SelectionItem::Point(points[3]),
            ]
        );

        editor.select_item(
            SelectionItem::Curve(spans[0]),
            Modifiers {
                control: true,
                ..Modifiers::default()
            },
        );
        assert_eq!(
            editor.selection(),
            &[
                SelectionItem::Point(points[1]),
                SelectionItem::Point(points[0]),
                SelectionItem::Curve(spans[1]),
                SelectionItem::Point(points[2]),
                SelectionItem::Point(points[3]),
            ]
        );

        editor.select_item(SelectionItem::Curve(spans[0]), Modifiers::default());
        assert_eq!(editor.selection(), &[SelectionItem::Curve(spans[0])]);
        assert!(document.curve(spans[0].curve).is_some());
    }

    #[test]
    fn point_click_never_emits_drag_work_but_threshold_crossing_does() {
        let (document, _, points) = line_document();
        let scene = scene(&document);
        let endpoint = scene.viewport.model_to_screen([-4.0, 1.0]);
        let mut editor = ConstraintEditor::default();
        editor.pointer_down(
            &scene,
            pointer(9, endpoint.x, endpoint.y, Modifiers::default()),
        );
        assert_eq!(
            editor.active_pointer_gesture(),
            Some(ActivePointerGesture {
                pointer_id: 9,
                kind: ActivePointerGestureKind::Point,
            })
        );
        assert!(
            editor
                .pointer_move(
                    &scene,
                    pointer(9, endpoint.x + 2.9, endpoint.y, Modifiers::default())
                )
                .is_empty()
        );
        assert!(
            editor
                .pointer_up(
                    &scene,
                    scene.design_identity,
                    pointer(9, endpoint.x + 2.9, endpoint.y, Modifiers::default())
                )
                .is_empty()
        );
        assert_eq!(editor.active_pointer_gesture(), None);

        editor.pointer_down(
            &scene,
            pointer(10, endpoint.x, endpoint.y, Modifiers::default()),
        );
        let preview = editor.pointer_move(
            &scene,
            pointer(10, endpoint.x + 3.0, endpoint.y, Modifiers::default()),
        );
        assert_eq!(
            preview,
            vec![EditorEffect::RequestProjectedPointMove {
                pointer_id: 10,
                request_id: 0,
                point: points[0],
                model_position: [-3.94, 1.0],
            }]
        );
        assert_eq!(
            editor.pointer_up(
                &scene,
                scene.design_identity,
                pointer(10, endpoint.x + 4.0, endpoint.y, Modifiers::default())
            ),
            vec![EditorEffect::ClearPointPreview]
        );
    }

    #[test]
    fn circle_circumferences_drag_their_semantic_centers_without_pointer_jump() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let centers = [
            document.add_point("left", [-3.0, 0.0]).expect("left"),
            document.add_point("right", [3.0, 0.0]).expect("right"),
        ];
        let radii = [
            document
                .add_scalar(
                    "left radius",
                    1.0,
                    ScalarUnit::Length,
                    ScalarDomain::Positive,
                )
                .expect("radius"),
            document
                .add_scalar(
                    "right radius",
                    1.0,
                    ScalarUnit::Length,
                    ScalarDomain::Positive,
                )
                .expect("radius"),
        ];
        let circles = [
            document
                .add_curve(
                    "left roller",
                    CurveDefinition::Circle {
                        center: centers[0],
                        radius: radii[0],
                    },
                )
                .expect("circle"),
            document
                .add_curve(
                    "right roller",
                    CurveDefinition::Circle {
                        center: centers[1],
                        radius: radii[1],
                    },
                )
                .expect("circle"),
        ];
        let scene = scene(&document);

        for (index, ((center, circle), center_position)) in centers
            .into_iter()
            .zip(circles)
            .zip([[-3.0, 0.0], [3.0, 0.0]])
            .enumerate()
        {
            let circumference = scene
                .viewport
                .model_to_screen([center_position[0] + 1.0, center_position[1]]);
            let moved = ScreenPoint {
                x: circumference.x + 10.0,
                y: circumference.y - 5.0,
            };
            let pointer_id = u64::try_from(index + 1).expect("pointer");
            let mut editor = ConstraintEditor::default();
            let down = editor.pointer_down(
                &scene,
                pointer(
                    pointer_id,
                    circumference.x,
                    circumference.y,
                    Modifiers::default(),
                ),
            );
            assert_eq!(
                down,
                vec![EditorEffect::SelectionChanged(vec![SelectionItem::Curve(
                    CurveSpan {
                        curve: circle,
                        segment: 0,
                    }
                )])]
            );
            assert_eq!(
                editor.pointer_move(
                    &scene,
                    pointer(pointer_id, moved.x, moved.y, Modifiers::default())
                ),
                vec![EditorEffect::RequestProjectedPointMove {
                    pointer_id,
                    request_id: 0,
                    point: center,
                    model_position: [center_position[0] + 0.2, center_position[1] + 0.1],
                }]
            );
        }
    }

    #[test]
    fn circular_arc_span_drags_its_semantic_center_without_pointer_jump() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let center = document.add_point("center", [1.0, 2.0]).expect("center");
        let radius = document
            .add_scalar("radius", 2.0, ScalarUnit::Length, ScalarDomain::Positive)
            .expect("radius");
        let start_angle = document
            .add_scalar("start angle", 0.0, ScalarUnit::Angle, ScalarDomain::Finite)
            .expect("start angle");
        let end_angle = document
            .add_scalar(
                "end angle",
                std::f64::consts::FRAC_PI_2,
                ScalarUnit::Angle,
                ScalarDomain::Finite,
            )
            .expect("end angle");
        let arc = document
            .add_curve(
                "arc",
                CurveDefinition::CircularArc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                    sweep: DocumentArcSweep::CounterClockwise,
                },
            )
            .expect("arc");
        let scene = scene(&document);
        let curve = scene
            .curves
            .iter()
            .find(|curve| curve.span.curve == arc)
            .expect("scene arc");
        assert_eq!(curve.drag_handle_point, Some(center));
        let press = curve.screen_polyline[curve.screen_polyline.len() / 2];
        let moved = ScreenPoint {
            x: press.x + 10.0,
            y: press.y - 5.0,
        };
        let mut editor = ConstraintEditor::default();
        assert_eq!(
            editor.pointer_down(&scene, pointer(1, press.x, press.y, Modifiers::default())),
            vec![EditorEffect::SelectionChanged(vec![SelectionItem::Curve(
                CurveSpan::line(arc)
            )])]
        );
        assert!(matches!(
            editor
                .pointer_move(&scene, pointer(1, moved.x, moved.y, Modifiers::default()))
                .as_slice(),
            [EditorEffect::RequestProjectedPointMove {
                pointer_id: 1,
                request_id: 0,
                point,
                model_position,
            }] if *point == center
                && (model_position[0] - 1.2).abs() <= 1.0e-12
                && (model_position[1] - 2.1).abs() <= 1.0e-12
        ));
    }

    #[test]
    fn twin_roller_geometry_remains_draggable_through_visible_dimension_overlaps() {
        let fixture =
            geosolve_sketch::alpha_scenario(geosolve_sketch::AlphaScenarioKind::MotionCam, 1.0)
                .expect("motion cam");
        let geosolve_sketch::AlphaScenarioIds::MotionCam(ids) = fixture.ids else {
            unreachable!()
        };
        let scene = scene(&fixture.document);
        let rollers = [
            (ids.left_center, ids.left_circle),
            (ids.right_center, ids.right_circle),
        ];

        for (roller_index, (center, circle)) in rollers.into_iter().enumerate() {
            let model_center = fixture
                .document
                .point(center)
                .expect("roller center")
                .position;
            let center_screen = scene.viewport.model_to_screen(model_center);
            let circumference_screen = scene
                .viewport
                .model_to_screen([model_center[0] + 1.0, model_center[1]]);

            for (press_index, press) in [center_screen, circumference_screen]
                .into_iter()
                .enumerate()
            {
                let pointer_id =
                    u64::try_from(10 * roller_index + press_index + 1).expect("pointer identity");
                let mut editor = ConstraintEditor::default();
                let expected_item = if press_index == 0 {
                    SelectionItem::Point(center)
                } else {
                    SelectionItem::Curve(CurveSpan::line(circle))
                };
                assert_eq!(
                    editor.pointer_move(
                        &scene,
                        pointer(pointer_id, press.x, press.y, Modifiers::default()),
                    ),
                    vec![EditorEffect::HoverChanged(EditorHoverState {
                        target: Some(EditorHoverTarget::Geometry(expected_item)),
                        context_owner: Some(expected_item),
                    })],
                    "hover must predict the draggable click target through a dimension overlap",
                );
                let _ = editor.pointer_down(
                    &scene,
                    pointer(pointer_id, press.x, press.y, Modifiers::default()),
                );
                let gesture = editor
                    .point_gesture_snapshot()
                    .expect("roller press starts a point gesture");
                assert_eq!(gesture.pointer_id, pointer_id);
                assert_eq!(gesture.point, center);
                assert!(
                    editor.selection() == [expected_item],
                    "click must preserve the same predicted persistent target",
                );
            }
        }

        let (radius_dimension, label_anchor) = scene
            .annotations
            .iter()
            .find_map(|annotation| {
                if annotation.kind != SceneAnnotationKind::Radius {
                    return None;
                }
                let SceneAnnotationGeometry::RadialDimension { label_anchor, .. } =
                    &annotation.geometry
                else {
                    return None;
                };
                Some((annotation.item, *label_anchor))
            })
            .expect("left driving radius annotation");
        let mut editor = ConstraintEditor::default();
        let _ = editor.pointer_down(
            &scene,
            pointer(100, label_anchor.x, label_anchor.y, Modifiers::default()),
        );
        assert_eq!(editor.selection(), &[radius_dimension]);
        assert!(editor.point_gesture_snapshot().is_none());
    }

    #[test]
    fn m75_problem_forced_annotation_has_identical_hover_and_click_target() {
        let (mut document, spans, _) = line_document();
        let horizontal = document
            .add_constraint(
                "problem horizontal",
                DocumentConstraintDefinition::Horizontal { line: spans[0] },
            )
            .expect("horizontal constraint");
        let mut scene = scene(&document);
        let annotation = scene
            .annotations
            .iter_mut()
            .find(|annotation| annotation.item == SelectionItem::Constraint(horizontal))
            .expect("horizontal annotation");
        assert_eq!(annotation.visibility, SceneAnnotationVisibility::Contextual);
        let marker = ScreenPoint { x: 50.0, y: 50.0 };
        annotation.geometry = SceneAnnotationGeometry::Label {
            anchor: marker,
            leader_from: None,
        };
        let item = annotation.item;
        let occurrence = SceneAnnotationOccurrence {
            item,
            marker_index: None,
        };
        assert!(
            scene.hit_test(marker, PickTolerance::default()).is_none(),
            "the problem-forced occurrence must not gain visibility from an underlying owner",
        );

        let mut ordinary = ConstraintEditor::default();
        assert!(
            ordinary
                .pointer_move(&scene, pointer(1, marker.x, marker.y, Modifiers::default()))
                .is_empty()
        );

        let mut forced = ConstraintEditor::default();
        assert_eq!(
            forced.pointer_move_with_problem_items(
                &scene,
                pointer(2, marker.x, marker.y, Modifiers::default()),
                &[item],
            ),
            vec![EditorEffect::HoverChanged(EditorHoverState {
                target: Some(EditorHoverTarget::Annotation(occurrence)),
                context_owner: None,
            })]
        );
        assert_eq!(
            forced.pointer_down_with_problem_items(
                &scene,
                pointer(2, marker.x, marker.y, Modifiers::default()),
                &[item],
            ),
            vec![EditorEffect::SelectionChanged(vec![item])]
        );
        assert_eq!(forced.selection(), &[item]);
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one tolerance-boundary regression keeps the complete annotation precedence and occurrence tie contract together"
    )]
    fn m75_annotation_priority_and_occurrence_ties_are_explicit_at_tolerance_boundary() {
        let (mut document, spans, _) = line_document();
        let constraints = ["first", "second"].map(|label| {
            document
                .add_constraint(
                    label,
                    DocumentConstraintDefinition::Horizontal { line: spans[0] },
                )
                .expect("horizontal constraint")
        });
        let mut scene = scene(&document);
        let probe = ScreenPoint { x: 50.0, y: 50.0 };
        let tolerance = PickTolerance::default().annotation_pixels;
        scene.annotations.retain(|annotation| {
            constraints
                .map(SelectionItem::Constraint)
                .contains(&annotation.item)
        });
        for annotation in &mut scene.annotations {
            annotation.visibility = SceneAnnotationVisibility::Always;
            annotation.geometry = SceneAnnotationGeometry::Glyph {
                markers: vec![
                    SceneGlyphMarker {
                        anchor: ScreenPoint {
                            x: probe.x - tolerance,
                            y: probe.y,
                        },
                        leader_from: None,
                        rotation_radians: 0.0,
                    },
                    SceneGlyphMarker {
                        anchor: ScreenPoint {
                            x: probe.x + tolerance,
                            y: probe.y,
                        },
                        leader_from: None,
                        rotation_radians: 0.0,
                    },
                ],
            };
        }
        scene.annotations.reverse();
        let expected_item = constraints
            .map(SelectionItem::Constraint)
            .into_iter()
            .min()
            .expect("constraint item");
        let expected_occurrence = SceneAnnotationOccurrence {
            item: expected_item,
            marker_index: Some(0),
        };
        assert_eq!(
            scene.annotation_occurrence_hit_test(probe, PickTolerance::default(), &[], None, &[],),
            Some((expected_occurrence, tolerance))
        );
        assert!(
            scene
                .annotation_occurrence_hit_test(
                    ScreenPoint {
                        x: probe.x,
                        y: probe.y + 0.01,
                    },
                    PickTolerance::default(),
                    &[],
                    None,
                    &[],
                )
                .is_none(),
            "a marker just outside the inclusive annotation radius must miss",
        );

        let mut editor = ConstraintEditor::default();
        assert_eq!(
            editor.pointer_move(&scene, pointer(3, probe.x, probe.y, Modifiers::default()),),
            vec![EditorEffect::HoverChanged(EditorHoverState {
                target: Some(EditorHoverTarget::Annotation(expected_occurrence)),
                context_owner: None,
            })]
        );
        assert_eq!(
            editor.pointer_down(&scene, pointer(3, probe.x, probe.y, Modifiers::default()),),
            vec![EditorEffect::SelectionChanged(vec![expected_item])]
        );

        let origin = scene.viewport.model_to_screen([0.0, 0.0]);
        scene.annotations.truncate(1);
        scene.annotations[0].geometry = SceneAnnotationGeometry::Label {
            anchor: origin,
            leader_from: None,
        };
        let datum_overlap_item = scene.annotations[0].item;
        let mut datum_overlap = super::ConstraintEditor::default();
        assert_eq!(
            datum_overlap
                .pointer_move(&scene, pointer(4, origin.x, origin.y, Modifiers::default()),),
            vec![EditorEffect::HoverChanged(EditorHoverState {
                target: Some(EditorHoverTarget::Annotation(SceneAnnotationOccurrence {
                    item: datum_overlap_item,
                    marker_index: None,
                })),
                context_owner: None,
            })],
            "a visible annotation must precede the intrinsic datum fallback",
        );
    }

    #[test]
    fn m75_first_sample_uses_prospective_context_for_hover_click_parity() {
        let (mut document, spans, _) = line_document();
        let horizontal = document
            .add_constraint(
                "contextual horizontal",
                DocumentConstraintDefinition::Horizontal { line: spans[0] },
            )
            .expect("horizontal constraint");
        let mut scene = scene(&document);
        let position = scene.viewport.model_to_screen([0.0, 1.0]);
        let item = SelectionItem::Constraint(horizontal);
        let annotation = scene
            .annotations
            .iter_mut()
            .find(|annotation| annotation.item == item)
            .expect("horizontal annotation");
        assert_eq!(annotation.visibility, SceneAnnotationVisibility::Contextual);
        annotation.geometry = SceneAnnotationGeometry::Label {
            anchor: position,
            leader_from: None,
        };
        assert!(
            scene
                .annotation_occurrence_hit_test(position, PickTolerance::default(), &[], None, &[],)
                .is_none(),
            "the annotation must begin hidden without geometry context",
        );
        assert_eq!(
            scene
                .annotation_occurrence_hit_test(
                    position,
                    PickTolerance::default(),
                    &[],
                    Some(SelectionItem::Curve(spans[0])),
                    &[],
                )
                .map(|hit| hit.0.item),
            Some(item),
            "the passive curve at this sample must reveal the annotation",
        );
        assert!(
            scene
                .draggable_geometry_hit_test_with_policy(
                    position,
                    PickTolerance::default(),
                    GeometryInteractionPolicy::default(),
                )
                .is_none(),
            "the overlap must exercise annotation versus passive geometry",
        );

        let occurrence = SceneAnnotationOccurrence {
            item,
            marker_index: None,
        };
        let mut editor = ConstraintEditor::default();
        assert_eq!(
            editor.pointer_move(
                &scene,
                pointer(5, position.x, position.y, Modifiers::default()),
            ),
            vec![EditorEffect::HoverChanged(EditorHoverState {
                target: Some(EditorHoverTarget::Annotation(occurrence)),
                context_owner: None,
            })],
            "the first sample must predict the newly revealed annotation owner",
        );
        assert_eq!(
            editor.pointer_down(
                &scene,
                pointer(5, position.x, position.y, Modifiers::default()),
            ),
            vec![EditorEffect::SelectionChanged(vec![item])],
        );
        assert_eq!(editor.selection(), &[item]);
    }

    #[test]
    fn m75_pointer_context_clears_on_owner_lifecycle_remaps() {
        let (document, _, points) = line_document();
        let scene = scene(&document);
        let endpoint = scene.viewport.model_to_screen([-4.0, 1.0]);
        let input = pointer(5, endpoint.x, endpoint.y, Modifiers::default());
        let expected = EditorHoverState {
            target: Some(EditorHoverTarget::Geometry(SelectionItem::Point(points[0]))),
            context_owner: Some(SelectionItem::Point(points[0])),
        };
        let cleared = vec![EditorEffect::HoverChanged(EditorHoverState::default())];
        let mut editor = ConstraintEditor::default();

        assert_eq!(
            editor.pointer_move(&scene, input),
            vec![EditorEffect::HoverChanged(expected)]
        );
        assert_eq!(editor.activate_tool(EditorTool::Line), cleared);
        assert_eq!(editor.hover_state(), EditorHoverState::default());

        assert!(editor.activate_tool(EditorTool::Select).is_empty());
        let _ = editor.pointer_move(&scene, input);
        assert_eq!(editor.pointer_leave(), cleared);

        let _ = editor.pointer_move(&scene, input);
        assert_eq!(editor.invalidate_draft_inference(), cleared);

        let _ = editor.pointer_move(&scene, input);
        assert_eq!(
            editor.set_geometry_pick_scope(GeometryPickScope::Profile),
            cleared,
        );

        let _ = editor.pointer_move(&scene, input);
        assert_eq!(editor.cancel(), cleared);

        let session = RetainedSketchDocumentSession::new(
            document,
            geosolve_sketch::DocumentSolveRequest::default(),
            geosolve_sketch::SolverConfig::default(),
        )
        .expect("retained session");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let _ = coordinator.editor_mut().pointer_move(&scene, input);
        assert_eq!(coordinator.editor().hover_state(), expected);
        coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                geosolve_sketch::DocumentEdit::CreatePoint {
                    label: "accepted scene remap".into(),
                    position: [8.0, 8.0],
                },
            )
            .expect("accepted scene-changing edit");
        assert_eq!(
            coordinator.editor().hover_state(),
            EditorHoverState::default(),
            "coordinator publication must invalidate pointer state with the retired scene",
        );
    }

    #[test]
    fn m75_selection_visibility_change_revokes_the_previous_pointer_owner() {
        let (mut document, spans, _) = line_document();
        let horizontal = document
            .add_constraint(
                "selection-revealed horizontal",
                DocumentConstraintDefinition::Horizontal { line: spans[0] },
            )
            .expect("horizontal constraint");
        let mut scene = scene(&document);
        let position = scene.viewport.model_to_screen([0.0, -1.0]);
        let source = SelectionItem::Curve(spans[0]);
        let passive = SelectionItem::Curve(spans[1]);
        let annotation_item = SelectionItem::Constraint(horizontal);
        {
            let annotation = scene
                .annotations
                .iter_mut()
                .find(|annotation| annotation.item == annotation_item)
                .expect("horizontal annotation");
            assert_eq!(annotation.visibility, SceneAnnotationVisibility::Contextual);
            annotation.geometry = SceneAnnotationGeometry::Label {
                anchor: position,
                leader_from: None,
            };
            assert!(!annotation.is_visible(&[], Some(passive), &[]));
            assert!(annotation.is_visible(&[source], Some(passive), &[]));
        }
        let geometry_hover = vec![EditorEffect::HoverChanged(EditorHoverState {
            target: Some(EditorHoverTarget::Geometry(passive)),
            context_owner: Some(passive),
        })];
        let annotation_hover = EditorHoverState {
            target: Some(EditorHoverTarget::Annotation(SceneAnnotationOccurrence {
                item: annotation_item,
                marker_index: None,
            })),
            context_owner: None,
        };
        let input = pointer(8, position.x, position.y, Modifiers::default());

        let mut set_editor = ConstraintEditor::default();
        assert_eq!(set_editor.pointer_move(&scene, input), geometry_hover);
        set_editor.set_selection([source]);
        assert_eq!(
            set_editor.hover_state(),
            EditorHoverState::default(),
            "replacing selection must revoke a prediction made under old annotation visibility",
        );
        assert_eq!(
            set_editor.pointer_move(&scene, input),
            vec![EditorEffect::HoverChanged(annotation_hover)],
        );

        let mut click_editor = ConstraintEditor::default();
        assert_eq!(click_editor.pointer_move(&scene, input), geometry_hover);
        click_editor.select_item(source, Modifiers::default());
        assert_eq!(click_editor.hover_state(), EditorHoverState::default());
        assert_eq!(
            click_editor.pointer_down(&scene, input),
            vec![EditorEffect::SelectionChanged(vec![annotation_item])],
            "the newly visible annotation is the next shared resolver owner",
        );
        assert_eq!(click_editor.selection(), &[annotation_item]);
    }

    #[test]
    fn viewport_round_trip_and_invalid_inputs_are_explicit() {
        let viewport = Viewport::new([713.0, 411.0], [3.0, -8.0], 27.5).expect("viewport");
        let model = [-4.25, 13.5];
        let round_trip = viewport.screen_to_model(viewport.model_to_screen(model));
        assert!((round_trip[0] - model[0]).abs() < 1.0e-12);
        assert!((round_trip[1] - model[1]).abs() < 1.0e-12);
        assert!(matches!(
            Viewport::new([0.0, 411.0], [0.0, 0.0], 1.0),
            Err(EditorError::InvalidViewport)
        ));
        assert!(matches!(
            ConstraintEditor::new(PickTolerance::default(), f64::NAN),
            Err(EditorError::InvalidTolerance)
        ));

        for scale in [1.0e-6, 1.0, 1.0e6] {
            for center in [[0.0, 0.0], [1.0e6, -1.0e6]] {
                let viewport =
                    Viewport::new([1000.0, 700.0], center, scale).expect("scaled viewport");
                let model = [center[0] + 2.0 / scale, center[1] - 3.0 / scale];
                let round_trip = viewport.screen_to_model(viewport.model_to_screen(model));
                assert!(
                    round_trip
                        .into_iter()
                        .zip(model)
                        .all(|(actual, expected)| (actual - expected).abs() <= 1.0e-12)
                );
            }
        }
    }

    #[test]
    fn every_core_draft_has_exact_completion_and_cancellation() {
        let (document, _, _) = line_document();
        let scene = scene(&document);
        let mut editor = ConstraintEditor::default();
        let center = scene.viewport.model_to_screen([0.0, 0.0]);
        for tool in [
            EditorTool::Point,
            EditorTool::Line,
            EditorTool::Rectangle,
            EditorTool::Circle,
            EditorTool::CounterClockwiseArc,
        ] {
            editor.activate_tool(tool);
            let first =
                editor.pointer_down(&scene, pointer(1, center.x, center.y, Modifiers::default()));
            if tool == EditorTool::Point {
                assert!(matches!(
                    first.as_slice(),
                    [
                        EditorEffect::CommitConstruction {
                            expected,
                            proposal: ConstructionProposal::Point { .. },
                            ..
                        },
                        EditorEffect::ClearConstructionPreview
                    ] if *expected == scene.design_identity
                ));
                continue;
            }
            assert!(
                first
                    .iter()
                    .all(|effect| !matches!(effect, EditorEffect::CommitConstruction { .. }))
            );
            let second = scene.viewport.model_to_screen([2.0, 1.0]);
            let effects =
                editor.pointer_down(&scene, pointer(1, second.x, second.y, Modifiers::default()));
            if tool == EditorTool::CounterClockwiseArc {
                let third = scene.viewport.model_to_screen([0.0, 2.0]);
                let effects =
                    editor.pointer_down(&scene, pointer(1, third.x, third.y, Modifiers::default()));
                assert!(has_construction_commit(&effects));
                acknowledge_planned_commit(&mut editor, &effects);
            } else if tool != EditorTool::Point {
                assert!(has_construction_commit(&effects));
                acknowledge_planned_commit(&mut editor, &effects);
            }
            editor.activate_tool(tool);
            editor.pointer_down(&scene, pointer(2, center.x, center.y, Modifiers::default()));
            assert!(
                editor
                    .cancel()
                    .iter()
                    .all(|effect| !matches!(effect, EditorEffect::CommitConstruction { .. }))
            );
        }
        editor.activate_tool(EditorTool::Polyline);
        editor.pointer_down(&scene, pointer(3, center.x, center.y, Modifiers::default()));
        let second = scene.viewport.model_to_screen([2.0, 1.0]);
        editor.pointer_down(&scene, pointer(3, second.x, second.y, Modifiers::default()));
        let effects = editor.complete_draft(scene.design_identity);
        assert!(has_construction_commit(&effects));
        acknowledge_planned_commit(&mut editor, &effects);
    }

    #[test]
    fn accepted_geometry_remains_pickable_but_removed_design_ids_are_not_snappable_or_authoritative()
     {
        let (accepted, _, points) = line_document();
        let design = SketchDocument::new(10.0).expect("design");
        #[allow(clippy::default_trait_access)]
        let identity = geosolve_sketch::RetainedSketchDocumentSession::new(
            design.clone(),
            geosolve_sketch::DocumentSolveRequest::default(),
            Default::default(),
        )
        .expect("session")
        .design_identity();
        let scene = EditorScene::from_accepted_for_design(
            7,
            identity,
            &accepted,
            &design,
            Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport"),
            0.5,
        )
        .expect("scene");
        let old_point = scene.viewport.model_to_screen([-4.0, 1.0]);
        assert!(
            scene
                .hit_test(old_point, PickTolerance::default())
                .is_some_and(|hit| hit.item == SelectionItem::Point(points[0]))
        );
        let mut editor = ConstraintEditor::default();
        editor.activate_tool(EditorTool::Line);
        assert!(
            editor
                .pointer_down(
                    &scene,
                    pointer(1, old_point.x, old_point.y, Modifiers::default()),
                )
                .is_empty()
        );
        assert!(matches!(
            editor.draft.as_ref().map(|draft| draft.points.as_slice()),
            Some([ConstructionPoint::New(position)])
                if (position[0] + 4.0).abs() < 1.0e-12
                    && (position[1] - 1.0).abs() < 1.0e-12
        ));
        let end = scene.viewport.model_to_screen([-2.0, 1.0]);
        let effects = editor.pointer_down(&scene, pointer(1, end.x, end.y, Modifiers::default()));
        assert!(effects.is_empty());
        assert!(editor.pending_construction_commit_token().is_none());
    }

    #[test]
    fn snapped_operand_snapshot_keeps_preview_and_commit_branch_identical() {
        let (document, _, points) = line_document();
        let session = geosolve_sketch::RetainedSketchDocumentSession::new(
            document,
            geosolve_sketch::DocumentSolveRequest::default(),
            geosolve_sketch::SolverConfig::default(),
        )
        .expect("session");
        let accepted = session
            .accepted_state_for_current_input()
            .expect("accepted state");
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            accepted.design_identity(),
            accepted.document(),
            session.design_document(),
            Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport"),
            0.5,
        )
        .expect("accepted scene")
        .with_retained_session(&session)
        .expect("bound accepted scene");
        let mut retained_design = session.design_document().clone();
        retained_design
            .set_point_position(points[0], [-4.0, 5.0])
            .expect("different current coordinate after the operand snapshot");
        let mut editor = ConstraintEditor::default();
        editor.activate_tool(EditorTool::Line);
        let start = scene.viewport.model_to_screen([-4.0, 1.0]);
        let end = scene.viewport.model_to_screen([-2.0, 1.0]);
        editor.pointer_down(&scene, pointer(9, start.x, start.y, Modifiers::default()));
        let effects = editor.pointer_down(&scene, pointer(9, end.x, end.y, Modifiers::default()));
        let (_, plan) = construction_plan_effect(&effects);
        let proposal = &plan.proposal;
        let ConstructionProposal::Line {
            start: ConstructionPoint::Existing { id, position },
            ..
        } = proposal
        else {
            panic!("expected line snapped to an existing point");
        };
        assert_eq!(*id, points[0]);
        assert!((position[0] + 4.0).abs() < 1.0e-12);
        assert!((position[1] - 1.0).abs() < 1.0e-12);
        let result = proposal
            .apply(&mut retained_design)
            .expect("construction uses accepted operand snapshot");
        let CurveDefinition::Line {
            branch_direction, ..
        } = &retained_design
            .curve(result.curves[0])
            .expect("committed line")
            .definition
        else {
            panic!("expected line definition");
        };
        assert!((branch_direction[0] - 1.0).abs() < 1.0e-12);
        assert!(branch_direction[1].abs() < 1.0e-12);
    }

    #[test]
    fn preview_and_commit_share_identical_typed_operands() {
        let (document, _, _) = line_document();
        let scene = scene(&document);
        let mut editor = ConstraintEditor::default();
        editor.activate_tool(EditorTool::Circle);
        let center = scene.viewport.model_to_screen([0.0, 0.0]);
        assert!(matches!(
            editor
                .pointer_down(&scene, pointer(4, center.x, center.y, Modifiers::default()))
                .as_slice(),
            [EditorEffect::PreviewConstruction(
                ConstructionPreview::Anchor { .. }
            )]
        ));
        let radius = scene.viewport.model_to_screen([2.0, 0.0]);
        let preview =
            editor.pointer_move(&scene, pointer(4, radius.x, radius.y, Modifiers::default()));
        let commit =
            editor.pointer_down(&scene, pointer(4, radius.x, radius.y, Modifiers::default()));
        assert!(matches!((preview.as_slice(), commit.as_slice()),
            ([EditorEffect::PreviewConstruction(ConstructionPreview::Complete { proposal: first, geometry: ConstructionPreviewGeometry::Circle { center: resolved, radius } })], [EditorEffect::CommitConstruction { proposal: second, .. }, EditorEffect::ClearConstructionPreview])
                if first == second && *resolved == [0.0, 0.0] && (*radius - 2.0).abs() < 1.0e-12));
    }

    #[test]
    fn arc_preview_and_commit_share_the_editor_normalized_endpoint() {
        let (document, _, _) = line_document();
        let scene = scene(&document);
        let mut editor = ConstraintEditor::default();
        editor.activate_tool(EditorTool::CounterClockwiseArc);
        let center = scene.viewport.model_to_screen([0.0, 0.0]);
        let start = scene.viewport.model_to_screen([2.0, 0.0]);
        let off_radius_end = scene.viewport.model_to_screen([0.0, 5.0]);
        editor.pointer_down(&scene, pointer(4, center.x, center.y, Modifiers::default()));
        editor.pointer_down(&scene, pointer(4, start.x, start.y, Modifiers::default()));
        let preview = editor.pointer_move(
            &scene,
            pointer(4, off_radius_end.x, off_radius_end.y, Modifiers::default()),
        );
        let commit = editor.pointer_down(
            &scene,
            pointer(4, off_radius_end.x, off_radius_end.y, Modifiers::default()),
        );
        assert!(matches!((preview.as_slice(), commit.as_slice()),
            ([EditorEffect::PreviewConstruction(ConstructionPreview::Complete { proposal: ConstructionProposal::CounterClockwiseArc { end: preview_end, .. }, .. })], [EditorEffect::CommitConstruction { proposal: ConstructionProposal::CounterClockwiseArc { end: commit_end, .. }, .. }, EditorEffect::ClearConstructionPreview])
                if (preview_end[0] - commit_end[0]).abs() < 1.0e-12
                    && (preview_end[1] - commit_end[1]).abs() < 1.0e-12
                    && (preview_end[0]).abs() < 1.0e-12
                    && (preview_end[1] - 2.0).abs() < 1.0e-12));
    }

    #[test]
    fn arc_draft_publishes_anchor_radius_guide_and_normalized_completion_stages() {
        let (document, _, _) = line_document();
        let scene = scene(&document);
        let mut editor = ConstraintEditor::default();
        editor.activate_tool(EditorTool::CounterClockwiseArc);
        let center = scene.viewport.model_to_screen([0.0, 0.0]);
        let start = scene.viewport.model_to_screen([2.0, 0.0]);
        let end = scene.viewport.model_to_screen([0.0, 5.0]);

        assert!(matches!(
            editor
                .pointer_down(&scene, pointer(4, center.x, center.y, Modifiers::default()))
                .as_slice(),
            [EditorEffect::PreviewConstruction(
                ConstructionPreview::Anchor { .. }
            )]
        ));
        assert!(matches!(
            editor.pointer_move(&scene, pointer(4, start.x, start.y, Modifiers::default())).as_slice(),
            [EditorEffect::PreviewConstruction(ConstructionPreview::ArcRadiusGuide { start: guide_start, .. })]
                if (guide_start[0] - 2.0).abs() < 1.0e-12 && guide_start[1].abs() < 1.0e-12
        ));
        assert!(matches!(
            editor.pointer_down(&scene, pointer(4, start.x, start.y, Modifiers::default())).as_slice(),
            [EditorEffect::PreviewConstruction(ConstructionPreview::ArcRadiusGuide { start: guide_start, .. })]
                if (guide_start[0] - 2.0).abs() < 1.0e-12 && guide_start[1].abs() < 1.0e-12
        ));
        assert!(matches!(
            editor.pointer_move(&scene, pointer(4, end.x, end.y, Modifiers::default())).as_slice(),
            [EditorEffect::PreviewConstruction(ConstructionPreview::Complete { proposal: ConstructionProposal::CounterClockwiseArc { end: preview_end, .. }, geometry: ConstructionPreviewGeometry::CounterClockwiseArc { sweep_radians, large_arc, .. } })]
                if (preview_end[0]).abs() < 1.0e-12 && (preview_end[1] - 2.0).abs() < 1.0e-12
                    && (*sweep_radians - std::f64::consts::FRAC_PI_2).abs() < 1.0e-12 && !large_arc
        ));
    }

    #[test]
    fn arc_preview_publishes_explicit_major_counterclockwise_sweep() {
        let (document, _, _) = line_document();
        let scene = scene(&document);
        let mut editor = ConstraintEditor::default();
        editor.activate_tool(EditorTool::CounterClockwiseArc);
        let center = scene.viewport.model_to_screen([0.0, 0.0]);
        let start = scene.viewport.model_to_screen([2.0, 0.0]);
        let end = scene.viewport.model_to_screen([0.0, -4.0]);

        editor.pointer_down(&scene, pointer(4, center.x, center.y, Modifiers::default()));
        editor.pointer_down(&scene, pointer(4, start.x, start.y, Modifiers::default()));
        assert!(matches!(
            editor
                .pointer_move(&scene, pointer(4, end.x, end.y, Modifiers::default()))
                .as_slice(),
            [EditorEffect::PreviewConstruction(ConstructionPreview::Complete {
                geometry: ConstructionPreviewGeometry::CounterClockwiseArc {
                    end: normalized_end,
                    sweep_radians,
                    large_arc,
                    ..
                },
                ..
            })] if normalized_end[0].abs() < 1.0e-12
                && (normalized_end[1] + 2.0).abs() < 1.0e-12
                && (*sweep_radians - 3.0 * std::f64::consts::FRAC_PI_2).abs() < 1.0e-12
                && *large_arc
        ));
    }

    #[test]
    fn finish_commits_then_clears_the_polyline_preview() {
        let (document, _, _) = line_document();
        let scene = scene(&document);
        let mut editor = ConstraintEditor::default();
        editor.activate_tool(EditorTool::Polyline);
        assert!(!editor.can_complete_draft());
        let first = scene.viewport.model_to_screen([0.0, 0.0]);
        let second = scene.viewport.model_to_screen([2.0, 0.0]);
        editor.pointer_down(&scene, pointer(4, first.x, first.y, Modifiers::default()));
        assert!(!editor.can_complete_draft());
        editor.pointer_down(&scene, pointer(4, second.x, second.y, Modifiers::default()));
        assert!(editor.can_complete_draft());
        let effects = editor.complete_draft(scene.design_identity);
        assert!(effects.iter().any(|effect| matches!(
            effect,
            EditorEffect::CommitConstructionPlan {
                expected,
                plan: ConstructionCommitPlan {
                    proposal: ConstructionProposal::Polyline { .. },
                    ..
                },
                ..
            } if expected.design_identity() == scene.design_identity
        )));
        acknowledge_planned_commit(&mut editor, &effects);
    }

    #[test]
    fn proposal_apply_uses_public_document_construction_and_is_atomic() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let proposal = ConstructionProposal::CounterClockwiseArc {
            center: ConstructionPoint::New([0.0, 0.0]),
            start: [1.0, 0.0],
            end: [0.0, 1.0],
        };
        let result = proposal.apply(&mut document).expect("arc");
        assert_eq!(result.points.len(), 1);
        assert_eq!(result.scalars.len(), 3);
        assert_eq!(result.curves.len(), 1);
        assert!(matches!(
            document.curve(result.curves[0]).expect("curve").definition,
            CurveDefinition::CircularArc {
                sweep: DocumentArcSweep::CounterClockwise,
                ..
            }
        ));
        let before = document.clone();
        assert!(
            ConstructionProposal::Circle {
                center: ConstructionPoint::New([0.0, 0.0]),
                radius: 0.0
            }
            .apply(&mut document)
            .is_err()
        );
        assert_eq!(document, before);
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the all-family atomicity matrix is clearer as one directly comparable regression"
    )]
    fn advanced_curve_proposals_apply_atomically_through_public_document_geometry() {
        let conic = ConicConstructionOptions {
            minor_axis_ratio: 0.6,
            arc_start: -0.4,
            arc_end: 1.7,
            arc_sweep: DocumentArcSweep::Clockwise,
            middle_weight: 0.8,
            trim_start: -1.2,
            trim_end: 1.4,
            semi_conjugate: 1.3,
            hyperbola_branch: DocumentHyperbolaBranch::Negative,
        };
        let proposals = vec![
            ConstructionProposal::QuadraticBezier {
                controls: [
                    ConstructionPoint::New([0.0, 0.0]),
                    ConstructionPoint::New([1.0, 2.0]),
                    ConstructionPoint::New([3.0, 0.0]),
                ],
            },
            ConstructionProposal::CubicBezier {
                controls: [
                    ConstructionPoint::New([0.0, 0.0]),
                    ConstructionPoint::New([1.0, 2.0]),
                    ConstructionPoint::New([2.0, -1.0]),
                    ConstructionPoint::New([4.0, 0.0]),
                ],
            },
            ConstructionProposal::Ellipse {
                center: ConstructionPoint::New([0.0, 0.0]),
                major_axis_point: ConstructionPoint::New([3.0, 0.5]),
                minor_axis_ratio: conic.minor_axis_ratio,
            },
            ConstructionProposal::EllipticalArc {
                center: ConstructionPoint::New([0.0, 0.0]),
                major_axis_point: ConstructionPoint::New([3.0, 0.5]),
                minor_axis_ratio: conic.minor_axis_ratio,
                start_angle: conic.arc_start,
                end_angle: conic.arc_end,
                sweep: conic.arc_sweep,
            },
            ConstructionProposal::RationalQuadraticConic {
                start: ConstructionPoint::New([0.0, 0.0]),
                weighted_middle: [1.0, 2.0],
                middle_weight: conic.middle_weight,
                end: ConstructionPoint::New([3.0, 0.0]),
            },
            ConstructionProposal::Parabola {
                vertex: ConstructionPoint::New([0.0, 0.0]),
                focus: ConstructionPoint::New([1.0, 0.5]),
                trim_start: conic.trim_start,
                trim_end: conic.trim_end,
            },
            ConstructionProposal::Hyperbola {
                center: ConstructionPoint::New([0.0, 0.0]),
                transverse_axis_point: ConstructionPoint::New([2.0, 0.5]),
                semi_conjugate: conic.semi_conjugate,
                branch: conic.hyperbola_branch,
                trim_start: conic.trim_start,
                trim_end: conic.trim_end,
            },
            ConstructionProposal::Nurbs {
                controls: vec![
                    ConstructionPoint::New([0.0, 0.0]),
                    ConstructionPoint::New([1.0, 2.0]),
                    ConstructionPoint::New([3.0, 2.0]),
                    ConstructionPoint::New([4.0, 0.0]),
                ],
                options: NurbsConstructionOptions {
                    form: DocumentBSplineForm::Clamped,
                    degree: 3,
                    weights: vec![1.0, 0.8, 1.0, 1.2],
                    gauge_index: 2,
                },
            },
            ConstructionProposal::Nurbs {
                controls: vec![
                    ConstructionPoint::New([0.0, 0.0]),
                    ConstructionPoint::New([2.0, 0.0]),
                    ConstructionPoint::New([2.0, 2.0]),
                    ConstructionPoint::New([0.0, 2.0]),
                ],
                options: NurbsConstructionOptions {
                    form: DocumentBSplineForm::Periodic,
                    degree: 2,
                    weights: Vec::new(),
                    gauge_index: 0,
                },
            },
        ];

        for proposal in proposals {
            let mut document = SketchDocument::new(10.0).expect("document");
            let result = proposal.apply(&mut document).expect("advanced proposal");
            assert_eq!(result.curves.len(), 1);
            let curve = result.curves[0];
            #[allow(clippy::default_trait_access)]
            let session = geosolve_sketch::RetainedSketchDocumentSession::new(
                document.clone(),
                geosolve_sketch::DocumentSolveRequest::default(),
                Default::default(),
            )
            .expect("advanced proposal solves through the retained public session");
            assert!(session.accepted_state().is_some());
            let spans = document.curve_spans(curve).expect("semantic spans");
            assert!(!spans.is_empty());
            for span in spans {
                let intervals = document.visible_intervals(span).expect("visible intervals");
                assert!(!intervals.is_empty());
                for interval in intervals {
                    let parameter = (interval.start + interval.end) * 0.5;
                    let jet = document
                        .evaluate_curve_jet(span, parameter)
                        .expect("public curve evaluation");
                    assert!(jet.position.coords.iter().all(|value| value.is_finite()));
                    assert!(jet.first_derivative.iter().all(|value| value.is_finite()));
                }
            }
            if let CurveDefinition::Nurbs {
                form,
                controls,
                weights,
                gauge_weight,
                knots,
                span_ids,
                next_span_id,
                ..
            } = &document.curve(curve).expect("curve").definition
            {
                assert_eq!(controls.len(), weights.len());
                assert!(weights.contains(gauge_weight));
                assert_eq!(
                    span_ids.len(),
                    match form {
                        DocumentBSplineForm::Clamped => controls.len() - 3,
                        DocumentBSplineForm::Periodic => controls.len(),
                    }
                );
                assert_eq!(
                    *next_span_id,
                    u32::try_from(span_ids.len()).expect("bounded spans") + 1
                );
                assert!(knots.windows(2).all(|pair| pair[0] <= pair[1]));
            }
        }
    }

    #[test]
    fn invalid_advanced_options_and_topology_retain_editor_and_document_state() {
        let mut editor = ConstraintEditor::default();
        let conic_before = editor.conic_options();
        let mut invalid_conic = conic_before;
        invalid_conic.minor_axis_ratio = 0.0;
        assert!(matches!(
            editor.set_conic_options(invalid_conic),
            Err(EditorError::InvalidConstructionOptions(_))
        ));
        assert_eq!(editor.conic_options(), conic_before);

        let nurbs_before = editor.nurbs_options().clone();
        for options in [
            NurbsConstructionOptions {
                degree: 0,
                ..nurbs_before.clone()
            },
            NurbsConstructionOptions {
                weights: vec![1.0, 0.0],
                ..nurbs_before.clone()
            },
            NurbsConstructionOptions {
                weights: vec![1.0, 1.0],
                gauge_index: 2,
                ..nurbs_before.clone()
            },
        ] {
            assert!(matches!(
                editor.set_nurbs_options(options),
                Err(EditorError::InvalidConstructionOptions(_))
            ));
            assert_eq!(editor.nurbs_options(), &nurbs_before);
        }

        let mut document = SketchDocument::new(10.0).expect("document");
        let before = document.clone();
        for options in [
            NurbsConstructionOptions {
                form: DocumentBSplineForm::Clamped,
                degree: 3,
                weights: Vec::new(),
                gauge_index: 0,
            },
            NurbsConstructionOptions {
                form: DocumentBSplineForm::Periodic,
                degree: 2,
                weights: vec![1.0, 1.0],
                gauge_index: 0,
            },
            NurbsConstructionOptions {
                form: DocumentBSplineForm::Clamped,
                degree: 2,
                weights: vec![1.0, 1.0, 1.0],
                gauge_index: 3,
            },
        ] {
            let controls = vec![
                ConstructionPoint::New([0.0, 0.0]),
                ConstructionPoint::New([1.0, 1.0]),
                ConstructionPoint::New([2.0, 0.0]),
            ];
            assert!(
                ConstructionProposal::Nurbs { controls, options }
                    .apply(&mut document)
                    .is_err()
            );
            assert_eq!(document, before);
        }
    }

    #[test]
    fn advanced_drafts_preview_public_curve_geometry_and_nurbs_finishes_explicitly() {
        let document = SketchDocument::new(10.0).expect("document");
        let scene = scene(&document);
        let click = |editor: &mut ConstraintEditor, pointer_id, model: [f64; 2]| {
            let screen = scene.viewport.model_to_screen(model);
            editor.pointer_down(
                &scene,
                pointer(pointer_id, screen.x, screen.y, Modifiers::default()),
            )
        };

        let mut bezier = ConstraintEditor::default();
        bezier.activate_tool(EditorTool::QuadraticBezier);
        click(&mut bezier, 1, [0.0, 0.0]);
        click(&mut bezier, 1, [1.0, 2.0]);
        let effects = click(&mut bezier, 1, [3.0, 0.0]);
        let EditorEffect::CommitConstruction { proposal, .. } = &effects[0] else {
            panic!("quadratic completion");
        };
        let mut applied = document.clone();
        let result = proposal
            .apply(&mut applied)
            .expect("preview proposal applies");
        let span = applied.curve_spans(result.curves[0]).expect("spans")[0];
        let start = applied
            .evaluate_curve_jet(span, 0.0)
            .expect("public start")
            .position;
        let end = applied
            .evaluate_curve_jet(span, 1.0)
            .expect("public end")
            .position;
        let mut preview_editor = ConstraintEditor::default();
        preview_editor.activate_tool(EditorTool::QuadraticBezier);
        click(&mut preview_editor, 2, [0.0, 0.0]);
        click(&mut preview_editor, 2, [1.0, 2.0]);
        let end_screen = scene.viewport.model_to_screen([3.0, 0.0]);
        let preview = preview_editor.pointer_move(
            &scene,
            pointer(2, end_screen.x, end_screen.y, Modifiers::default()),
        );
        assert!(matches!(
            preview.as_slice(),
            [EditorEffect::PreviewConstruction(ConstructionPreview::Complete {
                geometry: ConstructionPreviewGeometry::AdvancedCurve {
                    kind: AdvancedConstructionKind::QuadraticBezier,
                    curve_points,
                    ..
                },
                ..
            })] if curve_points.first() == Some(&[start.x, start.y])
                && curve_points.last() == Some(&[end.x, end.y])
                && curve_points.len()
                    == usize::from(ADVANCED_CURVE_PREVIEW_SUBDIVISIONS) + 1
                && curve_points.iter().flatten().all(|value| value.is_finite())
        ));

        let mut nurbs = ConstraintEditor::default();
        nurbs
            .set_nurbs_options(NurbsConstructionOptions {
                form: DocumentBSplineForm::Periodic,
                degree: 2,
                weights: vec![1.0, 0.8, 1.0, 1.2],
                gauge_index: 2,
            })
            .expect("NURBS options");
        nurbs.activate_tool(EditorTool::Nurbs);
        for point in [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]] {
            click(&mut nurbs, 3, point);
        }
        assert!(nurbs.can_complete_draft());
        assert!(matches!(
            nurbs.complete_draft(scene.design_identity).as_slice(),
            [
                EditorEffect::CommitConstruction {
                    proposal: ConstructionProposal::Nurbs { controls, options },
                    ..
                },
                EditorEffect::ClearConstructionPreview
            ] if controls.len() == 4
                && options.form == DocumentBSplineForm::Periodic
                && options.gauge_index == 2
        ));
    }

    #[test]
    fn line_construction_publishes_normalized_explicit_branches() {
        let mut line_document = SketchDocument::new(10.0).expect("document");
        let line = ConstructionProposal::Line {
            start: ConstructionPoint::New([0.0, 0.0]),
            end: ConstructionPoint::New([2.0, 1.0]),
        }
        .apply(&mut line_document)
        .expect("non-unit line delta is normalized");
        let CurveDefinition::Line {
            branch_direction, ..
        } = &line_document
            .curve(line.curves[0])
            .expect("line")
            .definition
        else {
            panic!("expected line");
        };
        assert!((branch_direction[0].hypot(branch_direction[1]) - 1.0).abs() < 1.0e-12);

        let mut polyline_document = SketchDocument::new(10.0).expect("document");
        let polyline = ConstructionProposal::Polyline {
            points: vec![
                ConstructionPoint::New([0.0, 0.0]),
                ConstructionPoint::New([2.0, 1.0]),
                ConstructionPoint::New([2.0, 4.0]),
            ],
        }
        .apply(&mut polyline_document)
        .expect("polyline deltas are normalized");
        let CurveDefinition::Polyline {
            branch_directions, ..
        } = &polyline_document
            .curve(polyline.curves[0])
            .expect("polyline")
            .definition
        else {
            panic!("expected polyline");
        };
        assert!(
            branch_directions
                .iter()
                .all(|direction| { (direction[0].hypot(direction[1]) - 1.0).abs() < 1.0e-12 })
        );

        let mut rectangle_document = SketchDocument::new(10.0).expect("document");
        let rectangle = ConstructionProposal::Rectangle {
            first: [2.0, 3.0],
            second: [-1.0, -2.0],
        }
        .apply(&mut rectangle_document)
        .expect("rectangle corner order is normalized");
        assert_eq!(rectangle.curves.len(), 4);
    }

    #[test]
    fn projected_drag_retains_last_valid_preview_and_requires_matching_pointer() {
        let (document, _, points) = line_document();
        let scene = scene(&document);
        let endpoint = scene.viewport.model_to_screen([-4.0, 1.0]);
        let mut editor = ConstraintEditor::default();
        editor.pointer_down(
            &scene,
            pointer(8, endpoint.x, endpoint.y, Modifiers::default()),
        );
        let request = editor.pointer_move(
            &scene,
            pointer(8, endpoint.x + 3.0, endpoint.y, Modifiers::default()),
        );
        assert!(matches!(
            request.as_slice(),
            [EditorEffect::RequestProjectedPointMove { request_id: 0, .. }]
        ));
        assert!(
            editor
                .projected_drag_result(9, 0, points[0], Some([3.0, 3.0]))
                .is_empty()
        );
        assert!(
            editor
                .projected_drag_result(8, 1, points[0], Some([3.0, 3.0]))
                .is_empty()
        );
        assert_eq!(
            editor.projected_drag_result(8, 0, points[0], Some([2.0, 3.0])),
            vec![EditorEffect::PreviewPointMove {
                point: points[0],
                model_position: [2.0, 3.0]
            }]
        );
        assert!(
            editor
                .projected_drag_result(8, 0, points[0], Some([f64::NAN, 0.0]))
                .is_empty()
        );
        assert!(
            matches!(editor.pointer_up(&scene, scene.design_identity, pointer(8, endpoint.x + 4.0, endpoint.y, Modifiers::default())).as_slice(),
            [EditorEffect::CommitPointMove { expected, point, model_position }] if *expected == scene.design_identity && *point == points[0] && (model_position[0] - 2.0).abs() < 1.0e-12 && (model_position[1] - 3.0).abs() < 1.0e-12)
        );

        editor.pointer_down(
            &scene,
            pointer(8, endpoint.x, endpoint.y, Modifiers::default()),
        );
        let request = editor.pointer_move(
            &scene,
            pointer(8, endpoint.x + 3.0, endpoint.y, Modifiers::default()),
        );
        assert!(matches!(
            request.as_slice(),
            [EditorEffect::RequestProjectedPointMove { request_id: 1, .. }]
        ));
        assert!(
            editor
                .projected_drag_result(8, 1, points[0], None)
                .is_empty()
        );
        assert_eq!(
            editor.pointer_up(
                &scene,
                scene.design_identity,
                pointer(8, endpoint.x + 4.0, endpoint.y, Modifiers::default())
            ),
            vec![EditorEffect::ClearPointPreview]
        );

        editor.pointer_down(
            &scene,
            pointer(8, endpoint.x, endpoint.y, Modifiers::default()),
        );
        editor.pointer_move(
            &scene,
            pointer(8, endpoint.x + 3.0, endpoint.y, Modifiers::default()),
        );
        assert_eq!(
            editor.projected_drag_result(8, 2, points[0], Some([7.0, 8.0])),
            vec![EditorEffect::PreviewPointMove {
                point: points[0],
                model_position: [7.0, 8.0]
            }]
        );
        editor.pointer_move(
            &scene,
            pointer(8, endpoint.x + 5.0, endpoint.y, Modifiers::default()),
        );
        assert!(
            editor
                .projected_drag_result(8, 2, points[0], Some([9.0, 9.0]))
                .is_empty()
        );
        assert!(
            editor
                .projected_drag_result(8, 3, points[0], None)
                .is_empty()
        );
        assert!(
            matches!(editor.pointer_up(&scene, scene.design_identity, pointer(8, endpoint.x + 5.0, endpoint.y, Modifiers::default())).as_slice(),
            [EditorEffect::CommitPointMove { model_position, .. }] if (model_position[0] - 7.0).abs() < 1.0e-12 && (model_position[1] - 8.0).abs() < 1.0e-12)
        );
    }

    #[test]
    fn tool_switch_interrupts_drag_and_closes_continuation_state() {
        let (document, _, points) = line_document();
        let scene = scene(&document);
        let endpoint = scene.viewport.model_to_screen([-4.0, 1.0]);
        let mut editor = ConstraintEditor::default();
        editor.pointer_down(
            &scene,
            pointer(1, endpoint.x, endpoint.y, Modifiers::default()),
        );
        editor.pointer_move(
            &scene,
            pointer(1, endpoint.x + 3.0, endpoint.y, Modifiers::default()),
        );
        assert_eq!(
            editor.projected_drag_result(1, 0, points[0], Some([1.0, 2.0])),
            vec![EditorEffect::PreviewPointMove {
                point: points[0],
                model_position: [1.0, 2.0]
            }]
        );
        assert_eq!(
            editor.activate_tool(EditorTool::Line),
            vec![EditorEffect::ClearPointPreview]
        );
        assert!(
            editor
                .pointer_up(
                    &scene,
                    scene.design_identity,
                    pointer(1, endpoint.x + 4.0, endpoint.y, Modifiers::default())
                )
                .is_empty()
        );

        editor.activate_tool(EditorTool::Select);
        editor.pointer_down(
            &scene,
            pointer(2, endpoint.x, endpoint.y, Modifiers::default()),
        );
        editor.pointer_move(
            &scene,
            pointer(2, endpoint.x + 3.0, endpoint.y, Modifiers::default()),
        );
        assert_eq!(
            editor.activate_tool(EditorTool::Circle),
            vec![EditorEffect::ClearPointPreview]
        );
    }

    #[test]
    fn foreign_pointer_down_interrupts_drag_without_an_old_release_commit() {
        let (document, _, points) = line_document();
        let scene = scene(&document);
        let first = scene.viewport.model_to_screen([-4.0, 1.0]);
        let second = scene.viewport.model_to_screen([4.0, 1.0]);
        let mut editor = ConstraintEditor::default();
        editor.pointer_down(&scene, pointer(1, first.x, first.y, Modifiers::default()));
        editor.pointer_move(
            &scene,
            pointer(1, first.x + 3.0, first.y, Modifiers::default()),
        );
        editor.projected_drag_result(1, 0, points[0], Some([1.0, 2.0]));
        assert_eq!(
            editor.pointer_down(&scene, pointer(2, second.x, second.y, Modifiers::default())),
            vec![
                EditorEffect::ClearPointPreview,
                EditorEffect::SelectionChanged(vec![SelectionItem::Point(points[1])]),
            ]
        );
        assert!(
            editor
                .pointer_up(
                    &scene,
                    scene.design_identity,
                    pointer(1, first.x + 4.0, first.y, Modifiers::default())
                )
                .is_empty()
        );
        assert!(
            editor
                .pointer_up(
                    &scene,
                    scene.design_identity,
                    pointer(2, second.x, second.y, Modifiers::default()),
                )
                .is_empty()
        );
    }

    #[test]
    fn draft_transition_matrix_covers_tools_stages_modifiers_and_interruption() {
        let (document, _, _) = line_document();
        let scene = scene(&document);
        let positions = [[0.0, 0.0], [2.0, 1.0], [0.0, 2.0]];
        let cases = [
            (EditorTool::Point, 1, false),
            (EditorTool::Line, 2, false),
            (EditorTool::Polyline, 2, true),
            (EditorTool::Rectangle, 2, false),
            (EditorTool::Circle, 2, false),
            (EditorTool::CounterClockwiseArc, 3, false),
        ];
        let modifiers = [
            Modifiers::default(),
            Modifiers {
                shift: true,
                ..Modifiers::default()
            },
            Modifiers {
                control: true,
                ..Modifiers::default()
            },
            Modifiers {
                command: true,
                ..Modifiers::default()
            },
        ];
        for (tool, stages, explicit_completion) in cases {
            for modifier in modifiers {
                let mut editor = ConstraintEditor::default();
                editor.activate_tool(tool);
                for (stage, model) in positions.into_iter().take(stages).enumerate() {
                    let screen = scene.viewport.model_to_screen(model);
                    let effects =
                        editor.pointer_down(&scene, pointer(7, screen.x, screen.y, modifier));
                    assert!(
                        editor
                            .pointer_up(
                                &scene,
                                scene.design_identity,
                                pointer(7, screen.x, screen.y, modifier),
                            )
                            .is_empty()
                    );
                    assert_eq!(
                        has_construction_commit(&effects),
                        stage + 1 == stages && !explicit_completion
                    );
                }
                let completed = if explicit_completion {
                    editor.complete_draft(scene.design_identity)
                } else {
                    Vec::new()
                };
                assert_eq!(has_construction_commit(&completed), explicit_completion);
            }
        }
        let mut editor = ConstraintEditor::default();
        editor.activate_tool(EditorTool::Line);
        let first = scene.viewport.model_to_screen(positions[0]);
        editor.pointer_down(&scene, pointer(7, first.x, first.y, Modifiers::default()));
        assert_eq!(
            editor.activate_tool(EditorTool::Circle),
            vec![EditorEffect::ClearConstructionPreview]
        );
        assert!(editor.cancel().is_empty());
    }

    #[test]
    fn degenerate_terminal_candidates_rollback_and_a_valid_retry_completes() {
        let (document, _, _) = line_document();
        let scene = scene(&document);
        let click = |editor: &mut ConstraintEditor, point: [f64; 2]| {
            let screen = scene.viewport.model_to_screen(point);
            editor.pointer_down(&scene, pointer(5, screen.x, screen.y, Modifiers::default()))
        };
        for (tool, invalid, valid) in [
            (EditorTool::Line, [0.0, 0.0], [2.0, 0.0]),
            (EditorTool::Rectangle, [2.0, 0.0], [2.0, 1.0]),
            (EditorTool::Circle, [0.0, 0.0], [2.0, 0.0]),
        ] {
            let mut editor = ConstraintEditor::default();
            editor.activate_tool(tool);
            let first = click(&mut editor, [0.0, 0.0]);
            assert_eq!(
                first.iter().any(|effect| matches!(
                    effect,
                    EditorEffect::PreviewConstruction(ConstructionPreview::Anchor { .. })
                )),
                tool == EditorTool::Circle
            );
            assert!(click(&mut editor, invalid).is_empty());
            let effects = click(&mut editor, valid);
            assert!(has_construction_commit(&effects));
        }

        let mut arc = ConstraintEditor::default();
        arc.activate_tool(EditorTool::CounterClockwiseArc);
        click(&mut arc, [0.0, 0.0]);
        assert!(click(&mut arc, [0.0, 0.0]).is_empty());
        click(&mut arc, [2.0, 0.0]);
        assert!(click(&mut arc, [0.0, 0.0]).is_empty());
        assert!(has_construction_commit(&click(&mut arc, [0.0, 2.0])));

        let mut polyline = ConstraintEditor::default();
        polyline.activate_tool(EditorTool::Polyline);
        click(&mut polyline, [0.0, 0.0]);
        assert!(click(&mut polyline, [0.0, 0.0]).is_empty());
        click(&mut polyline, [2.0, 0.0]);
        let effects = polyline.complete_draft(scene.design_identity);
        assert!(effects.iter().any(|effect| match effect {
            EditorEffect::CommitConstruction {
                proposal: ConstructionProposal::Polyline { points },
                ..
            }
            | EditorEffect::CommitConstructionPlan {
                plan:
                    ConstructionCommitPlan {
                        proposal: ConstructionProposal::Polyline { points },
                        ..
                    },
                ..
            } => points.len() == 2,
            _ => false,
        }));
    }

    #[test]
    fn zero_sweep_arc_endpoint_rolls_back_to_center_and_start() {
        let (document, _, _) = line_document();
        let scene = scene(&document);
        let mut editor = ConstraintEditor::default();
        editor.activate_tool(EditorTool::CounterClockwiseArc);
        let click = |editor: &mut ConstraintEditor, point: [f64; 2]| {
            let screen = scene.viewport.model_to_screen(point);
            editor.pointer_down(&scene, pointer(6, screen.x, screen.y, Modifiers::default()))
        };
        click(&mut editor, [0.0, 0.0]);
        click(&mut editor, [2.0, 0.0]);
        assert!(click(&mut editor, [5.0, 0.0]).is_empty());
        assert!(click(&mut editor, [0.0, 2.0]).iter().any(|effect| matches!(
            effect,
            EditorEffect::CommitConstruction {
                proposal: ConstructionProposal::CounterClockwiseArc { .. },
                ..
            }
        )));
    }

    #[test]
    fn nonfinite_inputs_and_modifiers_do_not_disturb_normalized_state() {
        let (document, _, points) = line_document();
        let scene = scene(&document);
        let endpoint = scene.viewport.model_to_screen([-4.0, 1.0]);
        let mut editor = ConstraintEditor::default();
        editor.pointer_down(
            &scene,
            pointer(1, endpoint.x, endpoint.y, Modifiers::default()),
        );
        let selected = editor.selection().to_vec();
        assert_eq!(selected, vec![SelectionItem::Point(points[0])]);
        assert!(
            editor
                .pointer_down(
                    &scene,
                    pointer(2, f64::NAN, endpoint.y, Modifiers::default())
                )
                .is_empty()
        );
        assert_eq!(editor.selection(), selected);
        assert!(
            editor
                .pointer_move(
                    &scene,
                    pointer(1, f64::INFINITY, endpoint.y, Modifiers::default())
                )
                .is_empty()
        );
        assert!(
            editor
                .pointer_up(
                    &scene,
                    scene.design_identity,
                    pointer(1, endpoint.x, f64::NAN, Modifiers::default())
                )
                .is_empty()
        );

        editor.activate_tool(EditorTool::Line);
        let first = scene.viewport.model_to_screen([0.0, 0.0]);
        let second = scene.viewport.model_to_screen([2.0, 1.0]);
        let modifiers = Modifiers {
            shift: true,
            control: true,
            command: true,
        };
        assert!(
            editor
                .pointer_down(&scene, pointer(3, first.x, first.y, modifiers))
                .is_empty()
        );
        assert!(
            editor
                .pointer_down(&scene, pointer(3, f64::NAN, second.y, modifiers))
                .is_empty()
        );
        let effects = editor.pointer_down(&scene, pointer(3, second.x, second.y, modifiers));
        assert!(has_construction_commit(&effects));
    }
}
