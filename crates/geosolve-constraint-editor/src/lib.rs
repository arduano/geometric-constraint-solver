// SPDX-License-Identifier: GPL-3.0-or-later

//! Presentation-independent interaction state for 2D constraint editors.
//!
//! This crate consumes accepted public [`geosolve_sketch`] documents, produces
//! deterministic screen-space scene primitives, resolves pointer hits to persistent
//! sketch identities, and emits typed effects for a host to apply. It owns no solver
//! equations, renderer, DOM integration, persistence, or platform event loop.

mod annotations;
mod authoring;
mod coordinator;
mod feature_authoring;

pub use annotations::{
    SceneAnnotation, SceneAnnotationGeometry, SceneAnnotationKind, SceneAnnotationVisibility,
    SceneConstraintGlyph, SceneGlyphMarker,
};
pub use authoring::{
    AuthoringApplication, AuthoringOperand, AuthoringOperandKind, AuthoringOptions,
    AuthoringOutcome, AuthoringState, AuthoringTool, AuthoringWarning,
};
pub use coordinator::{
    ActionAvailability, ActionState, AuditDto, AuditProvenance, AuthoringMutation, BranchAction,
    ComputedFeatureMutation, ComputedFeatureProblemMetadata, ComputedProfileBoundary,
    ComputedSceneState, ContactBranchAction, CoordinatorActionKind, CoordinatorError,
    DimensionTargetDisplayUnit, DimensionTargetMetadata, DisabledReason, DisplayDimensionTarget,
    EditorMutation, EditorProblemCategory, EditorProblemMetadata, EditorProblemScope,
    EditorProblemTarget, FeatureAuthoringCornerBinding, FeatureAuthoringPointerDownOutcome,
    FeatureAuthoringPreview, FeatureAuthoringPreviewMetadata, FeatureAuthoringPreviewToken,
    FeatureAuthoringTransaction, LifecycleDto, LifecycleStatus, MeasurementPublication,
    MutationOutcome, ProblemsDto, ProjectedDragRejectionStage, ProjectedDragWorkEvidence,
    ReplayAction, RestoreCheckpoint, RetainedEditorCoordinator, display_dimension_target,
};
pub use feature_authoring::{
    FeatureAuthoringCandidate, FeatureAuthoringCornerPreview, FeatureAuthoringGuidance,
    FeatureAuthoringOptions, FeatureAuthoringOutcome, FeatureAuthoringPick, FeatureAuthoringStage,
    FeatureAuthoringState, FeatureAuthoringTool, FeatureAuthoringWarning,
    FeatureAuthoringWarningKind,
};
pub use geosolve_sketch::SketchAcceptedDocumentRedundancy;
pub use geosolve_sketch_features::{
    ComputedCircularArc, ComputedCornerRef, ComputedEdge, ComputedEdgeGeometry, ComputedEdgeId,
    ComputedEdgeProvenance, ComputedEvaluationAllocator, ComputedEvaluationAllocatorHighWater,
    ComputedEvaluationRevision, ComputedFeature, ComputedFeatureAllocatorHighWater,
    ComputedFeatureCornerId, ComputedFeatureDefinition, ComputedFeatureDocument,
    ComputedFeatureDocumentDigest, ComputedFeatureDocumentError, ComputedFeatureDocumentId,
    ComputedFeatureDocumentIdentity, ComputedFeatureEvaluation, ComputedFeatureEvaluationInput,
    ComputedFeatureEvaluationPolicy, ComputedFeatureEvaluationState, ComputedFeatureFailure,
    ComputedFeatureId, ComputedFeatureLifecycleHighWater, ComputedFeatureRevision,
    ComputedFeatureSnapshot, ComputedFilletContact, ComputedFilletCorner,
    ComputedFilletParentIndex, ComputedFilletSet, ComputedSourceInterval, NativeCurveSpanSource,
    NewComputedFilletCorner,
};
use std::cmp::Ordering;

use geosolve_sketch::{
    ContactDomain, ContactNeighborhood, CurveDefinition, CurveId, CurveSpan, DesignPointId,
    DesignScalarId, DocumentAngleOrientation, DocumentArcSweep, DocumentBSplineForm,
    DocumentConstraintDefinition, DocumentConstraintId, DocumentCurveContinuity,
    DocumentCurveCurvatureRelation, DocumentCurveNormalSide, DocumentCurveSpanRef,
    DocumentDimensionId, DocumentDimensionMode, DocumentEdit, DocumentHyperbolaBranch,
    DocumentObjectId, MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT, PreparedSketchInput, ScalarDomain,
    ScalarUnit, SketchDesignIdentity, SketchDocument, TangentOrientation,
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
        if !screen_size
            .into_iter()
            .all(|value| value.is_finite() && value > 0.0)
            || !model_center.into_iter().all(f64::is_finite)
            || !pixels_per_model_unit.is_finite()
            || pixels_per_model_unit <= 0.0
        {
            return Err(EditorError::InvalidViewport);
        }
        Ok(viewport)
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

/// Persistent selectable identity understood by the headless editor.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SelectionItem {
    Point(DesignPointId),
    Curve(CurveSpan),
    Constraint(DocumentConstraintId),
    Dimension(DocumentDimensionId),
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
            Self::Feature(_) | Self::FeatureCorner(_) => None,
        }
    }
}

/// One accepted point primitive for presentation and picking.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScenePoint {
    pub id: DesignPointId,
    pub model_position: [f64; 2],
    pub screen_position: ScreenPoint,
}

/// One accepted semantic curve span represented by a display polyline.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneCurve {
    pub span: CurveSpan,
    pub screen_polyline: Vec<ScreenPoint>,
    /// Curve parameters paired one-to-one with [`Self::screen_polyline`].
    pub screen_parameters: Vec<f64>,
    /// Optional semantic point moved when this visible curve is dragged.
    ///
    /// Selection remains curve-based. The handle only defines gesture ownership,
    /// so presentation adapters do not need to infer a circular curve's center.
    pub drag_handle_point: Option<DesignPointId>,
}

/// One evaluation-local computed curve with stable feature/corner selection.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneComputedCurve {
    pub edge: geosolve_sketch_features::ComputedEdgeId,
    pub owner: geosolve_sketch_features::ComputedCornerRef,
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
/// The variant order is semantic priority: contact handles win over the
/// radius grip/spoke/rail/arc, which wins over native accepted geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SceneFilletHit {
    Contact {
        handle: SceneFilletContactHandle,
        distance_pixels: f64,
    },
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
            Self::Contact { handle, .. } => SelectionItem::FeatureCorner(handle.owner),
            Self::Radius { owner, .. } => SelectionItem::FeatureCorner(owner),
            Self::Native(hit) => hit.item,
        }
    }
}

/// Deterministic presentation-neutral scene derived from one accepted revision.
#[derive(Clone, Debug, PartialEq)]
pub struct EditorScene {
    pub accepted_revision: u64,
    pub design_identity: SketchDesignIdentity,
    pub viewport: Viewport,
    pub points: Vec<ScenePoint>,
    pub curves: Vec<SceneCurve>,
    /// Generated Fillet arcs. Source replacement fragments remain native
    /// [`SceneCurve`] values so native span selection and dragging stay intact.
    pub computed_curves: Vec<SceneComputedCurve>,
    pub feature_identity: Option<geosolve_sketch_features::ComputedFeatureDocumentIdentity>,
    pub computed_input: Option<geosolve_sketch_features::ComputedFeatureEvaluationInput>,
    fillet_interaction_origin: Option<geosolve_sketch_features::ComputedFeatureEvaluationInput>,
    /// Explicit direct-manipulation affordances supplied for current Fillet corners.
    pub fillet_affordances: Vec<SceneFilletCornerAffordances>,
    /// Typed per-corner continuation limits. These remain available even when a
    /// fold/singularity prevents construction of a radius rail affordance.
    pub computed_fillet_continuation_statuses: Vec<ComputedFilletContinuationStatus>,
    /// Accepted, geometry-derived constraint and dimension presentation.
    pub annotations: Vec<SceneAnnotation>,
    construction_snap_points: Vec<ScenePoint>,
}

impl EditorScene {
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

    /// Builds the accepted visible scene while restricting construction snaps to
    /// point identities that still exist in the current retained design.
    ///
    /// Picking continues to use every accepted visible point. Only construction
    /// operand snapping crosses this design-topology boundary.
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
        document: &SketchDocument,
        snap_design: Option<&SketchDocument>,
        viewport: Viewport,
        chord_tolerance_pixels: f64,
    ) -> Result<Self, EditorError> {
        if !chord_tolerance_pixels.is_finite() || chord_tolerance_pixels <= 0.0 {
            return Err(EditorError::InvalidTolerance);
        }
        let points: Vec<_> = document
            .points()
            .iter()
            .map(|point| ScenePoint {
                id: point.id,
                model_position: point.position,
                screen_position: viewport.model_to_screen(point.position),
            })
            .collect();
        let construction_snap_points = points
            .iter()
            .copied()
            .filter(|point| snap_design.is_none_or(|design| design.point(point.id).is_some()))
            .collect();
        let mut curves = Vec::new();
        for curve in document.curves() {
            for span in document.curve_spans(curve.id)? {
                for interval in document.visible_intervals(span)? {
                    let start = document.evaluate_curve_jet(span, interval.start)?;
                    let end = document.evaluate_curve_jet(span, interval.end)?;
                    let start = viewport.model_to_screen([start.position.x, start.position.y]);
                    let end = viewport.model_to_screen([end.position.x, end.position.y]);
                    let mut screen_polyline = vec![start];
                    let mut screen_parameters = vec![interval.start];
                    tessellate(
                        document,
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
        let annotations = annotations::build_annotations(document, &points, &curves, viewport);
        Ok(Self {
            accepted_revision,
            design_identity,
            viewport,
            points,
            curves,
            computed_curves: Vec::new(),
            feature_identity: None,
            computed_input: None,
            fillet_interaction_origin: None,
            fillet_affordances: Vec::new(),
            computed_fillet_continuation_statuses: Vec::new(),
            annotations,
            construction_snap_points,
        })
    }

    /// Builds one composite scene from exact-stamped accepted sketch and computed
    /// output. Replaced native supports use evaluated source fragments, while
    /// generated arcs retain stable feature/corner selection provenance.
    ///
    /// # Errors
    ///
    /// Rejects stale/mismatched provenance or non-finite generated geometry.
    #[allow(clippy::too_many_arguments)]
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
                ) => scene.curves.push(scene_curve_for_interval(
                    accepted_document,
                    viewport,
                    source.span,
                    interval.start,
                    interval.end,
                    chord_tolerance_pixels,
                )?),
                (
                    geosolve_sketch_features::ComputedEdgeGeometry::CircularArc(arc),
                    geosolve_sketch_features::ComputedEdgeProvenance::FilletArc { owner, .. },
                ) => scene.computed_curves.push(SceneComputedCurve {
                    edge: edge.id,
                    owner: *owner,
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
        scene.curves.sort_by_key(|curve| curve.span);
        scene.computed_curves.sort_by_key(|curve| curve.edge);
        scene.feature_identity = Some(computed.input().features);
        scene.computed_input = Some(computed.input());
        scene.fillet_affordances.clear();
        scene.annotations = annotations::build_annotations(
            accepted_document,
            &scene.points,
            &scene.curves,
            viewport,
        );
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
    /// Canvas painted identity is a non-authoritative hint. Contact and radius
    /// affordances keep their higher hit priority, then branch controls are
    /// independently tested against their paired model-space geometry. An
    /// accessible target skips pointer proximity but retains identical scene,
    /// owner, action and applicability checks.
    #[must_use]
    pub fn resolve_fillet_action(
        &self,
        input: SceneFilletActionInput,
        tolerance: PickTolerance,
    ) -> Option<SceneFilletActionTarget> {
        if !tolerance.is_valid() {
            return None;
        }
        match input {
            SceneFilletActionInput::Accessible(target) => {
                self.validated_fillet_action(&target).map(|_| target)
            }
            SceneFilletActionInput::Canvas { position, painted } => {
                if !position.is_finite()
                    || matches!(
                        self.resolve_fillet_hit(position, tolerance),
                        Some(SceneFilletHit::Contact { .. } | SceneFilletHit::Radius { .. })
                    )
                {
                    return None;
                }
                let expected = self.computed_input?;
                let model_position = self.viewport.screen_to_model(position);
                let (resolved, _) = self
                    .fillet_affordances
                    .iter()
                    .flat_map(|affordances| &affordances.actions)
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
    /// Contact handles win over radius grip/spoke/rail/arc hits. Those explicit
    /// Fillet affordances win over native accepted points and curves. Constraint
    /// and dimension annotations remain a separate presentation layer.
    #[must_use]
    pub fn resolve_fillet_hit(
        &self,
        position: ScreenPoint,
        tolerance: PickTolerance,
    ) -> Option<SceneFilletHit> {
        if !position.is_finite() || !tolerance.is_valid() {
            return None;
        }
        if let Some((handle, distance_pixels)) = self
            .fillet_affordances
            .iter()
            .flat_map(|affordances| affordances.contacts)
            .filter_map(|handle| {
                let distance = position.distance(handle.screen_position);
                (distance <= tolerance.point_pixels).then_some((handle, distance))
            })
            .min_by(|first, second| {
                first
                    .1
                    .total_cmp(&second.1)
                    .then_with(|| first.0.owner.cmp(&second.0.owner))
                    .then_with(|| {
                        fillet_parent_order(first.0.parent)
                            .cmp(&fillet_parent_order(second.0.parent))
                    })
            })
        {
            return Some(SceneFilletHit::Contact {
                handle,
                distance_pixels,
            });
        }
        if let Some((owner, distance_pixels)) = self
            .fillet_affordances
            .iter()
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
        self.native_authoring_hit_test(position, tolerance)
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
        if !position.is_finite() || !tolerance.is_valid() {
            return None;
        }
        let point_hit = self
            .points
            .iter()
            .filter_map(|point| point_hit(point, position, tolerance.point_pixels))
            .min_by(compare_hits);
        if point_hit.is_some() {
            return point_hit;
        }
        let native = self
            .curves
            .iter()
            .filter_map(|curve| curve_hit(curve, position, tolerance.curve_pixels))
            .min_by(compare_hits);
        let computed = self
            .computed_curves
            .iter()
            .filter_map(|curve| computed_curve_hit(curve, position, tolerance.curve_pixels))
            .min_by(compare_hits);
        match (native, computed) {
            (Some(first), Some(second)) => Some(if compare_hits(&first, &second).is_le() {
                first
            } else {
                second
            }),
            (Some(hit), None) | (None, Some(hit)) => Some(hit),
            (None, None) => None,
        }
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
        if !position.is_finite() || !tolerance.is_valid() {
            return None;
        }
        self.points
            .iter()
            .filter_map(|point| point_hit(point, position, tolerance.point_pixels))
            .min_by(compare_hits)
            .or_else(|| {
                self.curves
                    .iter()
                    .filter_map(|curve| curve_hit(curve, position, tolerance.curve_pixels))
                    .min_by(compare_hits)
            })
    }

    /// Returns bounded native authoring hits in deterministic interaction order.
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
    pub(crate) fn native_authoring_hit_candidates(
        &self,
        position: ScreenPoint,
        tolerance: PickTolerance,
        maximum_candidates: usize,
    ) -> Result<Vec<Hit>, NativeAuthoringHitError> {
        if !position.is_finite() || !tolerance.is_valid() {
            return Ok(Vec::new());
        }
        let candidates = self
            .points
            .iter()
            .filter_map(|point| point_hit(point, position, tolerance.point_pixels))
            .chain(
                self.curves
                    .iter()
                    .filter_map(|curve| curve_hit(curve, position, tolerance.curve_pixels)),
            );
        let mut unique = std::collections::BTreeMap::<SelectionItem, Hit>::new();
        for hit in candidates {
            if let Some(existing) = unique.get_mut(&hit.item) {
                if compare_hits(&hit, existing).is_lt() {
                    *existing = hit;
                }
                continue;
            }
            if unique.len() >= maximum_candidates {
                return Err(NativeAuthoringHitError::CandidateLimitExceeded { maximum_candidates });
            }
            unique.insert(hit.item, hit);
        }
        let mut hits = unique.into_values().collect::<Vec<_>>();
        hits.sort_by(|first, second| {
            native_hit_priority(first.item)
                .cmp(&native_hit_priority(second.item))
                .then_with(|| compare_hits(first, second))
        });
        Ok(hits)
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
        let hit = self.hit_test(position, tolerance)?;
        if !document_contains_item(source, hit.item) {
            return None;
        }
        let foreground_blocks = match hit.item {
            SelectionItem::Point(_) => self.points.iter().any(|point| {
                !document_contains_item(source, SelectionItem::Point(point.id))
                    && point_hit(point, position, tolerance.point_pixels)
                        .is_some_and(|candidate| candidate.distance_pixels <= hit.distance_pixels)
            }),
            SelectionItem::Curve(_) => self.curves.iter().any(|curve| {
                !document_contains_item(source, SelectionItem::Curve(curve.span))
                    && curve_hit(curve, position, tolerance.curve_pixels)
                        .is_some_and(|candidate| candidate.distance_pixels <= hit.distance_pixels)
            }),
            SelectionItem::Constraint(_)
            | SelectionItem::Dimension(_)
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
                annotation.is_visible(selection, visibility_context, problem_items)
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
            .min_by(|first, second| first.1.total_cmp(&second.1))
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
                annotation.is_visible(selection, Some(context_owner), problem_items)
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
pub struct Hit {
    pub item: SelectionItem,
    pub distance_pixels: f64,
    /// Explicit curve feature picked by the user, when the hit is a curve.
    pub curve_parameter: Option<f64>,
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
    Annotation(SceneAnnotationOccurrence),
}

impl EditorHoverTarget {
    /// Returns the persistent item represented by this proximity target.
    #[must_use]
    pub const fn item(self) -> SelectionItem {
        match self {
            Self::Geometry(item) => item,
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
    CommitPointMove {
        expected: SketchDesignIdentity,
        point: DesignPointId,
        model_position: [f64; 2],
    },
    ClearPointPreview,
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
    /// A complete construction proposal. Hosts apply this atomically with
    /// `SketchDocumentSession::transact`.
    CommitConstruction {
        expected: SketchDesignIdentity,
        proposal: ConstructionProposal,
    },
    /// A non-authoritative staged construction preview.
    PreviewConstruction(ConstructionPreview),
    ClearConstructionPreview,
    /// A non-authoritative preview of one explicitly staged inference candidate.
    PreviewInference(ProvisionalInferenceCandidate),
    /// Requests the revision-checked commit of one explicitly confirmed inference.
    CommitInference(ProvisionalInferenceCandidate),
    /// Clears the presentation of a staged inference candidate.
    ClearInferencePreview,
}

/// One explicitly staged, non-authoritative relation inference.
///
/// The candidate is bound to the exact retained design it was formed from. Its label is
/// presentation text only; the contained edit remains the sole proposed document change.
#[derive(Clone, Debug, PartialEq)]
pub struct ProvisionalInferenceCandidate {
    /// Retained design identity expected when this candidate is confirmed.
    pub expected: SketchDesignIdentity,
    /// Human-readable description for presentation.
    pub label: String,
    /// The ordinary sketch edit proposed by this candidate.
    pub edit: DocumentEdit,
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
/// deliberately separate from [`DocumentEdit`], whose single-edit shape cannot
/// refer to identities allocated by preceding point/scalar creations.
#[derive(Clone, Debug, PartialEq)]
pub enum ConstructionProposal {
    Point {
        position: [f64; 2],
    },
    Line {
        start: ConstructionPoint,
        end: ConstructionPoint,
    },
    Polyline {
        points: Vec<ConstructionPoint>,
    },
    Rectangle {
        first: [f64; 2],
        second: [f64; 2],
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
    EllipticalArc {
        center: ConstructionPoint,
        major_axis_point: ConstructionPoint,
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
    pub arc_start: f64,
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
        let mut candidate = document.clone();
        let result = self.apply_to(&mut candidate)?;
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
            Self::Point { position } => {
                result.points.push(document.add_point("point", *position)?);
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
            Self::Rectangle { first, second } => {
                let origin = [first[0].min(second[0]), first[1].min(second[1])];
                let width = (second[0] - first[0]).abs();
                let height = (second[1] - first[1]).abs();
                let ids = document.add_rectangle("rectangle", origin, width, height)?;
                result.points.extend(ids.points);
                result.curves.extend(ids.curves);
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

/// Configurable deterministic endpoint snapping policy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapTolerance {
    pub point_pixels: f64,
}

impl Default for SnapTolerance {
    fn default() -> Self {
        Self { point_pixels: 8.0 }
    }
}

impl SnapTolerance {
    fn is_valid(self) -> bool {
        self.point_pixels.is_finite() && self.point_pixels >= 0.0
    }
}

#[derive(Clone, Debug)]
struct Draft {
    tool: EditorTool,
    pointer_id: u64,
    points: Vec<ConstructionPoint>,
    positions: Vec<[f64; 2]>,
    conic_options: ConicConstructionOptions,
    nurbs_options: NurbsConstructionOptions,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct AnnotationHoverContext {
    owner: SelectionItem,
    origin: ScreenPoint,
}

/// Headless deterministic selection and point-gesture state machine.
#[derive(Clone, Debug)]
pub struct ConstraintEditor {
    selection: Vec<SelectionItem>,
    hover_target: Option<EditorHoverTarget>,
    hover_context: Option<AnnotationHoverContext>,
    curve_pick_parameters: Vec<(CurveSpan, f64)>,
    pick_tolerance: PickTolerance,
    drag_threshold_pixels: f64,
    point_gesture: Option<PointGesture>,
    feature_radius_gesture: Option<FeatureRadiusGesture>,
    feature_contact_gesture: Option<FeatureContactGesture>,
    computed_fillet_continuation_status: Option<ComputedFilletContinuationStatus>,
    fillet_branch_preview: Option<SceneFilletActionTarget>,
    tool: EditorTool,
    snap_tolerance: SnapTolerance,
    conic_options: ConicConstructionOptions,
    nurbs_options: NurbsConstructionOptions,
    draft: Option<Draft>,
    last_valid_drag_preview: Option<(u64, u64, u64, DesignPointId, [f64; 2])>,
    next_point_gesture_epoch: u64,
    next_projection_request: u64,
    staged_inference: Option<ProvisionalInferenceCandidate>,
}

impl Default for ConstraintEditor {
    fn default() -> Self {
        Self {
            selection: Vec::new(),
            hover_target: None,
            hover_context: None,
            curve_pick_parameters: Vec::new(),
            pick_tolerance: PickTolerance::default(),
            drag_threshold_pixels: 3.0,
            point_gesture: None,
            feature_radius_gesture: None,
            feature_contact_gesture: None,
            computed_fillet_continuation_status: None,
            fillet_branch_preview: None,
            tool: EditorTool::Select,
            snap_tolerance: SnapTolerance::default(),
            conic_options: ConicConstructionOptions::default(),
            nurbs_options: NurbsConstructionOptions::default(),
            draft: None,
            last_valid_drag_preview: None,
            next_point_gesture_epoch: 0,
            next_projection_request: 0,
            staged_inference: None,
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
        self.tool = tool;
        let mut effects = self.cancel_draft();
        effects.extend(self.cancel_point_gesture());
        effects.extend(self.cancel_feature_radius_gesture());
        effects.extend(self.cancel_feature_contact_gesture());
        effects.extend(self.clear_fillet_branch_preview());
        effects
    }

    /// Returns the active tool.
    #[must_use]
    pub const fn tool(&self) -> EditorTool {
        self.tool
    }

    /// Replaces the endpoint snap policy.
    ///
    /// # Errors
    ///
    /// Returns [`EditorError::InvalidTolerance`] when the pixel tolerance is invalid.
    pub fn set_snap_tolerance(&mut self, tolerance: SnapTolerance) -> Result<(), EditorError> {
        if !tolerance.is_valid() {
            return Err(EditorError::InvalidTolerance);
        }
        self.snap_tolerance = tolerance;
        Ok(())
    }

    /// Returns the configured snap policy.
    #[must_use]
    pub const fn snap_tolerance(&self) -> SnapTolerance {
        self.snap_tolerance
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
        self.conic_options = options;
        if let Some(draft) = self.draft.as_mut()
            && is_conic_tool(draft.tool)
        {
            draft.conic_options = options;
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
        self.nurbs_options = options.clone();
        if let Some(draft) = self.draft.as_mut()
            && draft.tool == EditorTool::Nurbs
        {
            draft.nurbs_options = options;
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
            .then(|| scene.resolve_fillet_action(input, self.pick_tolerance))
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
        let Some(target) = scene.resolve_fillet_action(input, self.pick_tolerance) else {
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
            scene.resolve_fillet_action(
                SceneFilletActionInput::Accessible(*target),
                self.pick_tolerance,
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

    /// Returns the explicit user-picked parameter for one selected curve span.
    #[must_use]
    pub fn curve_pick_parameter(&self, span: CurveSpan) -> Option<f64> {
        self.curve_pick_parameters
            .iter()
            .find_map(|(candidate, parameter)| (*candidate == span).then_some(*parameter))
    }

    /// Replaces ordered persistent selection, removing later duplicates.
    pub fn set_selection(&mut self, selection: impl IntoIterator<Item = SelectionItem>) {
        self.fillet_branch_preview = None;
        self.selection.clear();
        self.curve_pick_parameters.clear();
        for item in selection {
            if !self.selection.contains(&item) {
                self.selection.push(item);
            }
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
        self.fillet_branch_preview = None;
        if modifiers.extends_selection() {
            if let Some(index) = self.selection.iter().position(|selected| *selected == item) {
                self.selection.remove(index);
                if let SelectionItem::Curve(span) = item {
                    self.curve_pick_parameters
                        .retain(|(candidate, _)| *candidate != span);
                }
            } else {
                self.selection.push(item);
            }
        } else {
            self.selection.clear();
            self.curve_pick_parameters.clear();
            self.selection.push(item);
        }
    }

    /// Resolves a pointer press and changes selection immediately.
    pub fn pointer_down(&mut self, scene: &EditorScene, input: PointerInput) -> Vec<EditorEffect> {
        self.pointer_down_with_problem_items(scene, input, &[])
    }

    /// Resolves a pointer press while including diagnostically forced annotations.
    pub fn pointer_down_with_problem_items(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
        problem_items: &[SelectionItem],
    ) -> Vec<EditorEffect> {
        if !input.position.is_finite() {
            return Vec::new();
        }
        let mut effects = self.clear_fillet_branch_preview();
        if self.tool != EditorTool::Select {
            effects.extend(self.draft_down(scene, input));
            return effects;
        }
        if self.feature_radius_gesture.is_some() || self.feature_contact_gesture.is_some() {
            return effects;
        }
        let fillet_hit = scene.resolve_fillet_hit(input.position, self.pick_tolerance);
        if let Some(SceneFilletHit::Contact { handle, .. }) = fillet_hit {
            effects.extend(self.pointer_down_feature_contact_handle(scene, input, handle));
            return effects;
        }
        if let Some(SceneFilletHit::Radius {
            owner,
            distance_pixels,
        }) = fillet_hit
        {
            effects.extend(self.pointer_down_resolved_hit(
                scene,
                PointerInput {
                    modifiers: Modifiers::default(),
                    ..input
                },
                Some(Hit {
                    item: SelectionItem::FeatureCorner(owner),
                    distance_pixels,
                    curve_parameter: None,
                }),
            ));
            return effects;
        }
        let geometry_hit = match fillet_hit {
            Some(SceneFilletHit::Native(hit)) => Some(hit),
            Some(SceneFilletHit::Contact { .. } | SceneFilletHit::Radius { .. }) => unreachable!(),
            None => scene.hit_test(input.position, self.pick_tolerance),
        };
        let annotation_hit = scene.annotation_hit_test(
            input.position,
            self.pick_tolerance,
            &self.selection,
            self.visibility_context(),
            problem_items,
        );
        // A visible dimension leader may cross a point or semantic circle handle.
        // Preserve direct manipulation at that exact overlap; offset labels and
        // annotations over non-draggable geometry retain their existing priority.
        let hit = geometry_hit
            .filter(|hit| {
                scene.drag_handle_point(hit.item).is_some()
                    || scene.feature_radius_handle(hit.item).is_some()
            })
            .or(annotation_hit)
            .or(geometry_hit);
        effects.extend(self.pointer_down_resolved_hit(scene, input, hit));
        effects
    }

    /// Starts a computed-Fillet gesture for one explicitly painted preview arc.
    ///
    /// The coordinator validates current preview ownership and scene provenance
    /// before calling this path. This final editor-side check still requires the
    /// pointer to hit that exact owner's contact/radius affordance or, for an
    /// older scene without affordances, its computed curve. Contact wins over
    /// radius. A presentation target is an intent hint rather than a geometry
    /// oracle.
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
        let mut effects = match scene.resolve_fillet_hit(input.position, tolerance) {
            Some(SceneFilletHit::Contact { handle, .. }) if handle.owner == owner => {
                self.pointer_down_feature_contact_handle(scene, direct_input, handle)
            }
            Some(SceneFilletHit::Radius {
                owner: resolved,
                distance_pixels,
            }) if resolved == owner => self.pointer_down_resolved_hit(
                scene,
                direct_input,
                Some(Hit {
                    item: SelectionItem::FeatureCorner(owner),
                    distance_pixels,
                    curve_parameter: None,
                }),
            ),
            Some(SceneFilletHit::Contact { .. } | SceneFilletHit::Radius { .. }) => return None,
            Some(SceneFilletHit::Native(_)) | None => {
                let hit = scene
                    .computed_curves
                    .iter()
                    .find(|curve| curve.owner == owner)
                    .and_then(|curve| {
                        computed_curve_hit(curve, input.position, tolerance.curve_pixels)
                    })?;
                self.pointer_down_resolved_hit(scene, direct_input, Some(hit))
            }
        };
        let mut combined = self.clear_fillet_branch_preview();
        combined.append(&mut effects);
        Some(combined)
    }

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
                if self.selection.contains(&hit.item) {
                    self.curve_pick_parameters.push((span, parameter));
                }
            }
            if let Some((owner, radius, rail)) = scene.feature_radius_handle(hit.item)
                && self.selection.contains(&hit.item)
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
        let mut effects = self.clear_fillet_branch_preview();
        if self.tool != EditorTool::Select {
            effects.extend(self.draft_move(scene, input));
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
        if self.point_gesture.is_none() {
            effects.extend(self.move_hover(scene, input));
            return effects;
        }
        effects.extend(self.move_point_gesture(scene, input));
        effects
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

    fn move_hover(&mut self, scene: &EditorScene, input: PointerInput) -> Vec<EditorEffect> {
        if !input.position.is_finite() {
            return Vec::new();
        }
        if let Some(hit @ (SceneFilletHit::Contact { .. } | SceneFilletHit::Radius { .. })) =
            scene.resolve_fillet_hit(input.position, self.pick_tolerance)
        {
            let item = hit.item();
            return self.set_hover_state(
                Some(EditorHoverTarget::Geometry(item)),
                Some(AnnotationHoverContext {
                    owner: item,
                    origin: input.position,
                }),
            );
        }
        let annotation_hit = scene.annotation_occurrence_hit_test(
            input.position,
            self.pick_tolerance,
            &self.selection,
            self.visibility_context(),
            &[],
        );
        let geometry_hit = scene.hit_test(input.position, self.pick_tolerance);
        let (target, context) = if let Some((occurrence, _)) = annotation_hit {
            let context = self.hover_context.filter(|context| {
                scene.annotations.iter().any(|annotation| {
                    annotation.item == occurrence.item
                        && annotation.operands.contains(&context.owner)
                })
            });
            (Some(EditorHoverTarget::Annotation(occurrence)), context)
        } else if let Some(hit) = geometry_hit {
            (
                Some(EditorHoverTarget::Geometry(hit.item)),
                Some(AnnotationHoverContext {
                    owner: hit.item,
                    origin: input.position,
                }),
            )
        } else {
            let context = self.hover_context.filter(|context| {
                scene.contextual_annotation_transit(
                    input.position,
                    self.pick_tolerance,
                    &self.selection,
                    context.owner,
                    context.origin,
                    &[],
                )
            });
            (None, context)
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
        effects.extend(self.set_hover_state(None, None));
        effects
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
    pub fn pointer_up(
        &mut self,
        scene: &EditorScene,
        expected: SketchDesignIdentity,
        input: PointerInput,
    ) -> Vec<EditorEffect> {
        if self.tool != EditorTool::Select {
            return Vec::new();
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
                    vec![
                        EditorEffect::CommitPointMove {
                            expected,
                            point: gesture.point,
                            model_position: position,
                        },
                        EditorEffect::ClearPointPreview,
                    ]
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
        effects.extend(self.cancel_feature_radius_gesture());
        effects.extend(self.cancel_feature_contact_gesture());
        effects.extend(self.clear_fillet_branch_preview());
        effects.extend(self.set_hover_state(None, None));
        effects
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

    pub(crate) fn point_gesture_snapshot(&self) -> Option<PointGestureSnapshot> {
        self.point_gesture.map(|gesture| PointGestureSnapshot {
            epoch: gesture.epoch,
            pointer_id: gesture.pointer_id,
            point: gesture.point,
        })
    }

    /// Completes a variable-length polyline or NURBS draft.
    pub fn complete_draft(&mut self, expected: SketchDesignIdentity) -> Vec<EditorEffect> {
        let Some(draft) = self.draft.take() else {
            return Vec::new();
        };
        let proposal = match draft.tool {
            EditorTool::Polyline => polyline_proposal(&draft),
            EditorTool::Nurbs => nurbs_proposal(&draft),
            _ => None,
        };
        proposal
            .map(|proposal| commit_construction(expected, proposal))
            .unwrap_or_default()
    }

    /// Whether the current retained draft can be completed by an explicit Finish action.
    #[must_use]
    pub fn can_complete_draft(&self) -> bool {
        self.draft.as_ref().is_some_and(|draft| match draft.tool {
            EditorTool::Polyline => polyline_proposal(draft).is_some(),
            EditorTool::Nurbs => nurbs_proposal(draft).is_some(),
            _ => false,
        })
    }

    fn cancel_draft(&mut self) -> Vec<EditorEffect> {
        self.draft
            .take()
            .map(|_| vec![EditorEffect::ClearConstructionPreview])
            .unwrap_or_default()
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

    fn draft_down(&mut self, scene: &EditorScene, input: PointerInput) -> Vec<EditorEffect> {
        if !input.position.is_finite() {
            return Vec::new();
        }
        if self
            .draft
            .as_ref()
            .is_some_and(|draft| draft.pointer_id != input.pointer_id)
        {
            return Vec::new();
        }
        let position = scene.viewport.screen_to_model(input.position);
        if !position.into_iter().all(f64::is_finite) {
            return Vec::new();
        }
        let operand = snap_point(scene, input.position, self.snap_tolerance)
            .unwrap_or(ConstructionPoint::New(position));
        let prior_draft = self.draft.take();
        let mut draft = prior_draft.clone().unwrap_or(Draft {
            tool: self.tool,
            pointer_id: input.pointer_id,
            points: Vec::new(),
            positions: Vec::new(),
            conic_options: self.conic_options,
            nurbs_options: self.nurbs_options.clone(),
        });
        if draft.tool != self.tool {
            return Vec::new();
        }
        draft.points.push(operand);
        draft.positions.push(operand_position(operand));
        if !valid_draft_stage(&draft) {
            self.draft = prior_draft;
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
            || matches!(draft.tool, EditorTool::Polyline | EditorTool::Nurbs);
        if keep {
            let preview = draft_preview(&draft);
            self.draft = Some(draft);
            preview
                .map(EditorEffect::PreviewConstruction)
                .into_iter()
                .collect::<Vec<_>>()
        } else {
            proposal
                .map(|proposal| commit_construction(scene.design_identity, proposal))
                .unwrap_or_default()
        }
    }

    fn draft_move(&mut self, scene: &EditorScene, input: PointerInput) -> Vec<EditorEffect> {
        let Some(draft) = self.draft.as_ref() else {
            return Vec::new();
        };
        if draft.pointer_id != input.pointer_id || !input.position.is_finite() {
            return Vec::new();
        }
        let position = scene.viewport.screen_to_model(input.position);
        if !position.into_iter().all(f64::is_finite) {
            return Vec::new();
        }
        let operand = snap_point(scene, input.position, self.snap_tolerance)
            .unwrap_or(ConstructionPoint::New(position));
        let mut preview = draft.clone();
        preview.points.push(operand);
        preview.positions.push(operand_position(operand));
        draft_preview(&preview)
            .map(EditorEffect::PreviewConstruction)
            .into_iter()
            .collect()
    }

    /// Returns compatible core relation actions for the current ordered selection.
    #[must_use]
    pub fn available_constraints(&self, document: &SketchDocument) -> Vec<ConstraintKind> {
        available_constraints(document, &self.selection)
    }

    /// Produces one ordinary public sketch edit for a compatible relation action.
    ///
    /// # Errors
    ///
    /// Returns [`EditorError::IncompatibleConstraint`] if the selected operands do
    /// not exactly match the requested relation.
    pub fn constraint_edit(
        &self,
        document: &SketchDocument,
        kind: ConstraintKind,
        label: impl Into<String>,
    ) -> Result<DocumentEdit, EditorError> {
        constraint_edit(document, &self.selection, kind, label.into())
    }

    /// Stages one inference candidate without changing a document.
    ///
    /// Staging replaces any previously staged candidate and emits only its
    /// non-authoritative presentation preview.
    pub fn stage_inference(
        &mut self,
        candidate: ProvisionalInferenceCandidate,
    ) -> Vec<EditorEffect> {
        self.staged_inference = Some(candidate.clone());
        vec![EditorEffect::PreviewInference(candidate)]
    }

    /// Returns the one currently staged inference candidate, if any.
    #[must_use]
    pub fn staged_inference(&self) -> Option<&ProvisionalInferenceCandidate> {
        self.staged_inference.as_ref()
    }

    /// Cancels the staged inference candidate without changing a document.
    pub fn cancel_inference(&mut self) -> Vec<EditorEffect> {
        self.staged_inference
            .take()
            .map(|_| vec![EditorEffect::ClearInferencePreview])
            .unwrap_or_default()
    }

    /// Confirms and consumes the staged candidate, requesting its commit then preview clear.
    pub fn confirm_inference(&mut self) -> Vec<EditorEffect> {
        self.staged_inference
            .take()
            .map(|candidate| {
                vec![
                    EditorEffect::CommitInference(candidate),
                    EditorEffect::ClearInferencePreview,
                ]
            })
            .unwrap_or_default()
    }
}

fn commit_construction(
    expected: SketchDesignIdentity,
    proposal: ConstructionProposal,
) -> Vec<EditorEffect> {
    vec![
        EditorEffect::CommitConstruction { expected, proposal },
        EditorEffect::ClearConstructionPreview,
    ]
}

/// Complete M55 alpha relation action vocabulary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstraintKind {
    Fixed,
    Coincident,
    Horizontal,
    Vertical,
    PointOnCurve,
    Parallel,
    Perpendicular,
    EqualLength,
    EqualRadius,
    Midpoint,
    Symmetry,
    GenericContact,
    GenericTangency,
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
}

/// Exact persistent constraint family selected by contextual dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedConstraintKind {
    FixedPoint,
    CoincidentPoints,
    PointOnCurve,
    CurveContact,
    HorizontalLine,
    VerticalLine,
    ParallelLines,
    PerpendicularLines,
    RadialLine,
    EqualLength,
    EqualRadius,
    EqualCurvature,
    Midpoint,
    SymmetricAboutLine,
    CurveTangency,
    EndpointContinuity,
}

impl ResolvedConstraintKind {
    /// Selection-specific presentation label; equations remain domain-owned.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::FixedPoint => "Lock point",
            Self::CoincidentPoints => "Coincident",
            Self::PointOnCurve => "Point on curve",
            Self::CurveContact => "Curve contact",
            Self::HorizontalLine => "Horizontal",
            Self::VerticalLine => "Vertical",
            Self::ParallelLines => "Parallel",
            Self::PerpendicularLines => "Perpendicular",
            Self::RadialLine => "Normal to circle / arc",
            Self::EqualLength => "Equal length",
            Self::EqualRadius => "Equal radius",
            Self::EqualCurvature => "Equal curvature",
            Self::Midpoint => "Midpoint",
            Self::SymmetricAboutLine => "Symmetric about line",
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
    #[error("selected operands are incompatible with {0:?}")]
    IncompatibleConstraint(ConstraintKind),
    #[error("computed-feature snapshot does not match the supplied accepted sketch")]
    StaleComputedFeatureSnapshot,
    #[error("computed-feature interaction affordance is missing, stale, or malformed")]
    InvalidComputedFeatureAffordance,
    #[error(transparent)]
    Document(#[from] geosolve_sketch::DocumentError),
    #[error(transparent)]
    Curve(#[from] geosolve_sketch::DocumentCurveEvaluationError),
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
        screen_polyline,
        screen_parameters,
        drag_handle_point,
    })
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
    })
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
        | SelectionItem::Feature(_)
        | SelectionItem::FeatureCorner(_) => false,
    }
}

fn compare_hits(first: &Hit, second: &Hit) -> Ordering {
    first
        .distance_pixels
        .total_cmp(&second.distance_pixels)
        .then_with(|| first.item.cmp(&second.item))
}

const fn fillet_parent_order(parent: ComputedFilletParentIndex) -> u8 {
    match parent {
        ComputedFilletParentIndex::First => 0,
        ComputedFilletParentIndex::Second => 1,
    }
}

const fn native_hit_priority(item: SelectionItem) -> u8 {
    match item {
        SelectionItem::Point(_) => 0,
        SelectionItem::Curve(_) => 1,
        SelectionItem::Constraint(_)
        | SelectionItem::Dimension(_)
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
        | SelectionItem::Dimension(_) => false,
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

fn snap_point(
    scene: &EditorScene,
    position: ScreenPoint,
    tolerance: SnapTolerance,
) -> Option<ConstructionPoint> {
    if !tolerance.is_valid() {
        return None;
    }
    scene
        .construction_snap_points
        .iter()
        .filter_map(|point| {
            let distance = position.distance(point.screen_position);
            (distance <= tolerance.point_pixels).then_some((distance, *point))
        })
        .min_by(|first, second| {
            first
                .0
                .total_cmp(&second.0)
                .then_with(|| first.1.id.cmp(&second.1.id))
        })
        .map(|(_, point)| ConstructionPoint::Existing {
            id: point.id,
            position: point.model_position,
        })
}

fn operand_position(operand: ConstructionPoint) -> [f64; 2] {
    operand.position()
}

fn polyline_proposal(draft: &Draft) -> Option<ConstructionProposal> {
    (draft.points.len() >= 2 && draft.positions.windows(2).all(nonzero_segment)).then(|| {
        ConstructionProposal::Polyline {
            points: draft.points.clone(),
        }
    })
}

fn nurbs_proposal(draft: &Draft) -> Option<ConstructionProposal> {
    let minimum = usize::try_from(draft.nurbs_options.degree)
        .ok()?
        .checked_add(1)?;
    (draft.points.len() >= minimum
        && draft.positions.windows(2).all(nonzero_segment)
        && validate_nurbs_for_controls(&draft.nurbs_options, draft.points.len()).is_ok())
    .then(|| ConstructionProposal::Nurbs {
        controls: draft.points.clone(),
        options: draft.nurbs_options.clone(),
    })
}

fn valid_draft_stage(draft: &Draft) -> bool {
    match draft.tool {
        EditorTool::Point => draft.positions.len() == 1,
        EditorTool::Line | EditorTool::Rectangle | EditorTool::Circle => {
            draft.positions.len() < 2 || draft_proposal(draft).is_some()
        }
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
        EditorTool::Ellipse
        | EditorTool::EllipticalArc
        | EditorTool::Parabola
        | EditorTool::Hyperbola => draft.positions.len() < 2 || draft_proposal(draft).is_some(),
        EditorTool::Select => false,
    }
}

fn nonzero_segment(segment: &[[f64; 2]]) -> bool {
    let [start, end] = segment else {
        return false;
    };
    let length = (end[0] - start[0]).hypot(end[1] - start[1]);
    length.is_finite() && length > 0.0
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive table keeps every tool-to-proposal completion rule auditable"
)]
fn draft_proposal(draft: &Draft) -> Option<ConstructionProposal> {
    match draft.tool {
        EditorTool::Point => draft
            .positions
            .first()
            .copied()
            .map(|position| ConstructionProposal::Point { position }),
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
            if draft.points.len() == 2 && nonzero_segment(&draft.positions[..2]) =>
        {
            Some(ConstructionProposal::EllipticalArc {
                center: draft.points[0],
                major_axis_point: draft.points[1],
                minor_axis_ratio: draft.conic_options.minor_axis_ratio,
                start_angle: draft.conic_options.arc_start,
                end_angle: draft.conic_options.arc_end,
                sweep: draft.conic_options.arc_sweep,
            })
        }
        EditorTool::RationalQuadraticConic
            if draft.points.len() == 3
                && nonzero_segment(&[draft.positions[0], draft.positions[2]]) =>
        {
            Some(ConstructionProposal::RationalQuadraticConic {
                start: draft.points[0],
                weighted_middle: draft.positions[1],
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

fn draft_preview(draft: &Draft) -> Option<ConstructionPreview> {
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
        tool if advanced_kind(tool).is_some() && draft_proposal(draft).is_none() => {
            Some(ConstructionPreview::ControlPolygon {
                kind: advanced_kind(tool).expect("guarded advanced tool"),
                points: draft.positions.clone(),
            })
        }
        _ => complete_preview(draft),
    }
}

fn complete_preview(draft: &Draft) -> Option<ConstructionPreview> {
    let proposal = draft_proposal(draft)?;
    let geometry = match &proposal {
        ConstructionProposal::Point { position } => ConstructionPreviewGeometry::Point {
            position: *position,
        },
        ConstructionProposal::Line { .. } | ConstructionProposal::Polyline { .. } => {
            ConstructionPreviewGeometry::Polyline {
                points: draft.positions.clone(),
            }
        }
        ConstructionProposal::Rectangle { first, second } => {
            ConstructionPreviewGeometry::Rectangle {
                first: *first,
                second: *second,
            }
        }
        ConstructionProposal::Circle { radius, .. } => ConstructionPreviewGeometry::Circle {
            center: draft.positions[0],
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
        ConstructionProposal::QuadraticBezier { .. }
        | ConstructionProposal::CubicBezier { .. }
        | ConstructionProposal::Ellipse { .. }
        | ConstructionProposal::EllipticalArc { .. }
        | ConstructionProposal::RationalQuadraticConic { .. }
        | ConstructionProposal::Parabola { .. }
        | ConstructionProposal::Hyperbola { .. }
        | ConstructionProposal::Nurbs { .. } => {
            advanced_curve_preview(&proposal, &draft.positions, draft.tool)?
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
        ConstructionProposal::RationalQuadraticConic { middle_weight, .. } => {
            ConstructionProposal::RationalQuadraticConic {
                start: point(0)?,
                weighted_middle: *positions.get(1)?,
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

fn available_constraints(
    document: &SketchDocument,
    selection: &[SelectionItem],
) -> Vec<ConstraintKind> {
    match selection {
        [SelectionItem::Point(_)] => vec![ConstraintKind::Fixed],
        [SelectionItem::Point(_), SelectionItem::Point(_)] => vec![ConstraintKind::Coincident],
        [SelectionItem::Point(_), SelectionItem::Curve(span)]
        | [SelectionItem::Curve(span), SelectionItem::Point(_)]
            if supports_contact(document, *span) =>
        {
            let mut kinds = vec![ConstraintKind::PointOnCurve];
            if is_linear_span(document, *span) {
                kinds.push(ConstraintKind::Midpoint);
            }
            kinds
        }
        [SelectionItem::Curve(span)] if is_linear_span(document, *span) => {
            vec![ConstraintKind::Horizontal, ConstraintKind::Vertical]
        }
        [SelectionItem::Curve(first), SelectionItem::Curve(second)] => {
            let mut kinds = Vec::new();
            if is_linear_span(document, *first) && is_linear_span(document, *second) {
                kinds.extend([
                    ConstraintKind::Parallel,
                    ConstraintKind::Perpendicular,
                    ConstraintKind::EqualLength,
                ]);
            }
            if is_radius_curve(document, first.curve) && is_radius_curve(document, second.curve) {
                kinds.push(ConstraintKind::EqualRadius);
            }
            if supports_contact(document, *first) && supports_contact(document, *second) {
                kinds.extend([
                    ConstraintKind::GenericContact,
                    ConstraintKind::GenericTangency,
                ]);
            }
            kinds
        }
        [
            SelectionItem::Point(_),
            SelectionItem::Point(_),
            SelectionItem::Curve(line),
        ] if is_linear_span(document, *line) => vec![ConstraintKind::Symmetry],
        _ => Vec::new(),
    }
}

pub(crate) fn constraint_edit(
    document: &SketchDocument,
    selection: &[SelectionItem],
    kind: ConstraintKind,
    label: String,
) -> Result<DocumentEdit, EditorError> {
    if !available_constraints(document, selection).contains(&kind) {
        return Err(EditorError::IncompatibleConstraint(kind));
    }
    let definition = match (kind, selection) {
        (ConstraintKind::Fixed, [SelectionItem::Point(point)]) => {
            let target = document
                .point(*point)
                .ok_or(EditorError::IncompatibleConstraint(kind))?
                .position;
            DocumentConstraintDefinition::FixedPoint {
                point: *point,
                target,
            }
        }
        (
            ConstraintKind::Coincident,
            [SelectionItem::Point(first), SelectionItem::Point(second)],
        ) => DocumentConstraintDefinition::Coincident {
            first: *first,
            second: *second,
        },
        (ConstraintKind::Horizontal, [SelectionItem::Curve(line)]) => {
            DocumentConstraintDefinition::Horizontal { line: *line }
        }
        (ConstraintKind::Vertical, [SelectionItem::Curve(line)]) => {
            DocumentConstraintDefinition::Vertical { line: *line }
        }
        (ConstraintKind::Parallel, [SelectionItem::Curve(first), SelectionItem::Curve(second)]) => {
            DocumentConstraintDefinition::Parallel {
                first: *first,
                second: *second,
            }
        }
        (
            ConstraintKind::Perpendicular,
            [SelectionItem::Curve(first), SelectionItem::Curve(second)],
        ) => DocumentConstraintDefinition::Perpendicular {
            first: *first,
            second: *second,
        },
        (
            ConstraintKind::EqualLength,
            [SelectionItem::Curve(first), SelectionItem::Curve(second)],
        ) => DocumentConstraintDefinition::EqualLength {
            first: *first,
            second: *second,
        },
        (
            ConstraintKind::EqualRadius,
            [SelectionItem::Curve(first), SelectionItem::Curve(second)],
        ) => DocumentConstraintDefinition::EqualRadius {
            first: first.curve,
            second: second.curve,
        },
        (
            ConstraintKind::Midpoint,
            [SelectionItem::Point(point), SelectionItem::Curve(line)]
            | [SelectionItem::Curve(line), SelectionItem::Point(point)],
        ) => DocumentConstraintDefinition::Midpoint {
            point: *point,
            line: *line,
        },
        (
            ConstraintKind::Symmetry,
            [
                SelectionItem::Point(first),
                SelectionItem::Point(second),
                SelectionItem::Curve(line),
            ],
        ) => DocumentConstraintDefinition::SymmetricAboutLine {
            first: *first,
            second: *second,
            line: *line,
        },
        _ => return Err(EditorError::IncompatibleConstraint(kind)),
    };
    Ok(DocumentEdit::CreateConstraint { label, definition })
}

fn supports_contact(document: &SketchDocument, span: CurveSpan) -> bool {
    document.curve_contact_domains(span).is_ok()
}

fn is_radius_curve(document: &SketchDocument, curve: CurveId) -> bool {
    document.curve(curve).is_some_and(|curve| {
        matches!(
            curve.definition,
            CurveDefinition::Circle { .. } | CurveDefinition::CircularArc { .. }
        )
    })
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
    use geosolve_sketch::SketchDocument;

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
        let identity = geosolve_sketch::RetainedSketchDocumentSession::new(
            document.clone(),
            geosolve_sketch::DocumentSolveRequest::default(),
            Default::default(),
        )
        .expect("retained session")
        .design_identity();
        EditorScene::from_accepted(
            7,
            identity,
            document,
            Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport"),
            0.5,
        )
        .expect("scene")
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
    fn fillet_branch_resolution_rejects_spoofs_disabled_actions_and_higher_priority_hits() {
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
        assert_eq!(
            fixture.scene.resolve_fillet_action(
                SceneFilletActionInput::Canvas {
                    position: contact,
                    painted: Some(retained),
                },
                PickTolerance::default(),
            ),
            None,
            "contact handles retain priority over branch controls"
        );
        let radius = fixture.scene.fillet_affordances[0].radius_rail.screen_grip;
        assert_eq!(
            fixture.scene.resolve_fillet_action(
                SceneFilletActionInput::Canvas {
                    position: radius,
                    painted: Some(retained),
                },
                PickTolerance::default(),
            ),
            None,
            "radius affordances retain priority over branch controls"
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
    #[allow(clippy::too_many_lines)]
    fn fillet_affordances_validate_actions_and_share_contact_radius_native_priority() {
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
            Some(SceneFilletHit::Contact { handle, .. })
                if handle.parent == ComputedFilletParentIndex::First
                    && handle.owner == fixture.owner
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
    fn fillet_hover_and_pointer_down_resolve_the_same_contact_and_radius_owner() {
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
                kind: ActivePointerGestureKind::FilletContact,
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
        editor.pointer_down(
            &fixture.scene,
            pointer(
                72,
                handle.screen_position.x,
                handle.screen_position.y,
                Modifiers::default(),
            ),
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
                .pointer_down(
                    &fixture.scene,
                    pointer(
                        41,
                        handle.screen_position.x,
                        handle.screen_position.y,
                        Modifiers::default(),
                    )
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
        editor.pointer_down(
            &fixture.scene,
            pointer(
                82,
                handle.screen_position.x,
                handle.screen_position.y,
                Modifiers::default(),
            ),
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
        editor.pointer_down(&fixture.scene, down);
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

        editor.pointer_down(&fixture.scene, down);
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

        editor.pointer_down(&fixture.scene, down);
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

        let marker = match &horizontal_annotation.geometry {
            SceneAnnotationGeometry::Glyph { markers } => markers[0].anchor,
            _ => panic!("horizontal must be a glyph"),
        };
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
        assert_eq!(
            editor.pointer_leave(),
            vec![EditorEffect::HoverChanged(EditorHoverState::default())]
        );
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
                    .hit_test(*position, PickTolerance::default())
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
            scene.hit_test(bridge, PickTolerance::default()).is_none(),
            "off-leader bridge point must be outside geometry"
        );
        assert_eq!(
            hover_editor.pointer_move(
                &scene,
                pointer(10, bridge.x, bridge.y, Modifiers::default()),
            ),
            vec![EditorEffect::HoverChanged(context_state(None))]
        );

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

        let between_icons = ScreenPoint {
            x: (first_marker.anchor.x + second_marker.anchor.x) * 0.5,
            y: (first_marker.anchor.y + second_marker.anchor.y) * 0.5,
        };
        assert_eq!(
            hover_editor.pointer_move(
                &scene,
                pointer(10, between_icons.x, between_icons.y, Modifiers::default(),),
            ),
            vec![EditorEffect::HoverChanged(context_state(None))]
        );
        let second_occurrence = SceneAnnotationOccurrence {
            item: second_annotation.item,
            marker_index: Some(0),
        };
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
            )))]
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
            assert_eq!(
                marker.anchor,
                ScreenPoint {
                    x: (start.x + end.x) * 0.5,
                    y: (start.y + end.y) * 0.5,
                }
            );
            assert_ne!(marker.anchor, start);
            assert_ne!(marker.anchor, end);
            assert_eq!(marker.leader_from, None);
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

    fn assert_relation_edit_is_available(
        document: &SketchDocument,
        kind: ConstraintKind,
        selection: Vec<SelectionItem>,
    ) {
        let mut editor = ConstraintEditor::default();
        editor.set_selection(selection);
        assert!(editor.available_constraints(document).contains(&kind));
        assert!(editor.constraint_edit(document, kind, "relation").is_ok());
    }

    fn assert_relation_edit_is_rejected_without_mutation(
        document: &SketchDocument,
        before: &str,
        selection: Vec<SelectionItem>,
    ) {
        let mut editor = ConstraintEditor::default();
        editor.set_selection(selection);
        for kind in [
            ConstraintKind::Fixed,
            ConstraintKind::Coincident,
            ConstraintKind::Horizontal,
            ConstraintKind::Vertical,
            ConstraintKind::Parallel,
            ConstraintKind::Perpendicular,
            ConstraintKind::EqualLength,
        ] {
            assert!(matches!(
                editor.constraint_edit(document, kind, "invalid"),
                Err(EditorError::IncompatibleConstraint(actual)) if actual == kind
            ));
            assert_eq!(
                document.to_canonical_json().expect("canonical bytes"),
                before
            );
        }
    }

    #[test]
    fn line_is_selected_from_screen_space_without_dom_hit_targets() {
        let (document, spans, _) = line_document();
        let scene = scene(&document);
        for offset in [11.999, 12.0] {
            let hit = scene
                .hit_test(
                    ScreenPoint {
                        x: 500.0,
                        y: 300.0 + offset,
                    },
                    PickTolerance::default(),
                )
                .expect("line hit within the inclusive twelve-pixel radius");
            assert_eq!(hit.item, SelectionItem::Curve(spans[0]));
            assert!((hit.distance_pixels - offset).abs() < 1.0e-12);
            assert_eq!(hit.curve_parameter, Some(0.5));
        }
        assert!(
            scene
                .hit_test(
                    ScreenPoint {
                        x: 500.0,
                        y: 312.001,
                    },
                    PickTolerance::default(),
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
            })
        );
        for offset in [7.999, 8.0, 8.001] {
            assert_eq!(
                scene
                    .hit_test(
                        ScreenPoint {
                            x: endpoint.x + offset,
                            y: endpoint.y,
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
    fn extended_line_selection_exposes_and_builds_parallel_relation() {
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
        assert_eq!(
            editor.available_constraints(&document),
            vec![
                ConstraintKind::Parallel,
                ConstraintKind::Perpendicular,
                ConstraintKind::EqualLength,
                ConstraintKind::GenericContact,
                ConstraintKind::GenericTangency,
            ]
        );
        let edit = editor
            .constraint_edit(&document, ConstraintKind::Parallel, "parallel")
            .expect("compatible edit");
        assert!(matches!(
            edit,
            DocumentEdit::CreateConstraint {
                definition: DocumentConstraintDefinition::Parallel { first, second },
                ..
            } if first == spans[0] && second == spans[1]
        ));
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
    fn relation_applicability_matrix_builds_only_valid_public_edits() {
        let (mut document, spans, points) = line_document();
        let center = document.add_point("center", [0.0, 3.0]).expect("center");
        let radius = document
            .add_scalar("radius", 1.0, ScalarUnit::Length, ScalarDomain::Positive)
            .expect("radius");
        let circle = document
            .add_curve("circle", CurveDefinition::Circle { center, radius })
            .expect("circle");
        let circle_span = CurveSpan {
            curve: circle,
            segment: 0,
        };
        let cases = [
            (ConstraintKind::Fixed, vec![SelectionItem::Point(points[0])]),
            (
                ConstraintKind::Coincident,
                vec![
                    SelectionItem::Point(points[0]),
                    SelectionItem::Point(points[1]),
                ],
            ),
            (
                ConstraintKind::Horizontal,
                vec![SelectionItem::Curve(spans[0])],
            ),
            (
                ConstraintKind::Vertical,
                vec![SelectionItem::Curve(spans[0])],
            ),
            (
                ConstraintKind::Parallel,
                vec![
                    SelectionItem::Curve(spans[0]),
                    SelectionItem::Curve(spans[1]),
                ],
            ),
            (
                ConstraintKind::Perpendicular,
                vec![
                    SelectionItem::Curve(spans[0]),
                    SelectionItem::Curve(spans[1]),
                ],
            ),
            (
                ConstraintKind::EqualLength,
                vec![
                    SelectionItem::Curve(spans[0]),
                    SelectionItem::Curve(spans[1]),
                ],
            ),
        ];
        for (kind, selection) in cases {
            assert_relation_edit_is_available(&document, kind, selection);
        }

        let before = document.to_canonical_json().expect("canonical bytes");
        let invalid_selections = [
            vec![],
            vec![
                SelectionItem::Curve(spans[0]),
                SelectionItem::Point(points[0]),
            ],
            vec![
                SelectionItem::Point(points[0]),
                SelectionItem::Curve(spans[0]),
            ],
            vec![
                SelectionItem::Point(points[0]),
                SelectionItem::Point(points[1]),
                SelectionItem::Point(points[2]),
            ],
            vec![SelectionItem::Curve(circle_span)],
            vec![
                SelectionItem::Curve(spans[0]),
                SelectionItem::Curve(circle_span),
            ],
        ];
        for selection in invalid_selections {
            assert_relation_edit_is_rejected_without_mutation(&document, &before, selection);
        }
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
                    matches!(
                        editor.selection(),
                        [SelectionItem::Point(selected)] if *selected == center
                    ) || matches!(
                        editor.selection(),
                        [SelectionItem::Curve(selected)] if selected.curve == circle
                    )
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
                assert!(
                    editor
                        .pointer_down(&scene, pointer(1, third.x, third.y, Modifiers::default()))
                        .iter()
                        .any(|effect| matches!(
                            effect,
                            EditorEffect::CommitConstruction {
                                proposal: ConstructionProposal::CounterClockwiseArc { .. },
                                ..
                            }
                        ))
                );
            } else if tool != EditorTool::Point {
                assert!(
                    effects
                        .iter()
                        .any(|effect| matches!(effect, EditorEffect::CommitConstruction { .. }))
                );
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
        assert!(matches!(
            editor.complete_draft(scene.design_identity).as_slice(),
            [
                EditorEffect::CommitConstruction {
                    proposal: ConstructionProposal::Polyline { .. },
                    ..
                },
                EditorEffect::ClearConstructionPreview
            ]
        ));
    }

    #[test]
    fn snapping_is_identity_ordered_and_exactly_inclusive_at_tolerance() {
        let (document, _, points) = line_document();
        let scene = scene(&document);
        let endpoint = scene.viewport.model_to_screen([-4.0, 1.0]);
        let target = scene.viewport.model_to_screen([0.0, 0.0]);
        for offset in [7.999, 8.0, 8.001] {
            let mut editor = ConstraintEditor::default();
            editor
                .set_snap_tolerance(SnapTolerance { point_pixels: 8.0 })
                .expect("tolerance");
            editor.activate_tool(EditorTool::Line);
            editor.pointer_down(
                &scene,
                pointer(1, endpoint.x + offset, endpoint.y, Modifiers::default()),
            );
            let effects =
                editor.pointer_down(&scene, pointer(1, target.x, target.y, Modifiers::default()));
            assert_eq!(
                matches!(effects.as_slice(), [EditorEffect::CommitConstruction { proposal: ConstructionProposal::Line { start: ConstructionPoint::Existing { id, .. }, .. }, .. }, EditorEffect::ClearConstructionPreview] if *id == points[0]),
                offset <= 8.0,
            );
        }

        let midpoint = scene.viewport.model_to_screen([-4.0, 0.0]);
        let mut editor = ConstraintEditor::default();
        editor
            .set_snap_tolerance(SnapTolerance { point_pixels: 51.0 })
            .expect("tie tolerance");
        editor.activate_tool(EditorTool::Line);
        editor.pointer_down(
            &scene,
            pointer(2, midpoint.x, midpoint.y, Modifiers::default()),
        );
        let effects =
            editor.pointer_down(&scene, pointer(2, target.x, target.y, Modifiers::default()));
        let winner = points[0].min(points[2]);
        assert!(
            matches!(effects.as_slice(), [EditorEffect::CommitConstruction { proposal: ConstructionProposal::Line { start: ConstructionPoint::Existing { id, .. }, .. }, .. }, EditorEffect::ClearConstructionPreview] if *id == winner)
        );

        assert!(ConstraintEditor::new(PickTolerance::default(), -0.0).is_ok());
        assert!(
            editor
                .set_snap_tolerance(SnapTolerance {
                    point_pixels: f64::NAN
                })
                .is_err()
        );
    }

    #[test]
    fn accepted_geometry_remains_pickable_but_removed_design_ids_are_not_snappable() {
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
        editor.pointer_down(
            &scene,
            pointer(1, old_point.x, old_point.y, Modifiers::default()),
        );
        let end = scene.viewport.model_to_screen([-2.0, 1.0]);
        assert!(matches!(
            editor
                .pointer_down(&scene, pointer(1, end.x, end.y, Modifiers::default()))
                .as_slice(),
            [EditorEffect::CommitConstruction {
                proposal: ConstructionProposal::Line {
                    start: ConstructionPoint::New(position),
                    ..
                },
                ..
            }, EditorEffect::ClearConstructionPreview]
                if (position[0] + 4.0).abs() < 1.0e-12
                    && (position[1] - 1.0).abs() < 1.0e-12
        ));
    }

    #[test]
    fn snapped_operand_snapshot_keeps_preview_and_commit_branch_identical() {
        let (accepted, _, points) = line_document();
        let mut retained_design = accepted.clone();
        retained_design
            .set_point_position(points[0], [-4.0, 5.0])
            .expect("retained rejected-edit position");
        let accepted_scene = scene(&accepted);
        let scene = EditorScene::from_accepted_for_design(
            accepted_scene.accepted_revision,
            accepted_scene.design_identity,
            &accepted,
            &retained_design,
            accepted_scene.viewport,
            0.5,
        )
        .expect("accepted scene over divergent retained design");
        let mut editor = ConstraintEditor::default();
        editor.activate_tool(EditorTool::Line);
        let start = scene.viewport.model_to_screen([-4.0, 1.0]);
        let end = scene.viewport.model_to_screen([-2.0, 1.0]);
        editor.pointer_down(&scene, pointer(9, start.x, start.y, Modifiers::default()));
        let effects = editor.pointer_down(&scene, pointer(9, end.x, end.y, Modifiers::default()));
        let EditorEffect::CommitConstruction { proposal, .. } = &effects[0] else {
            panic!("expected terminal line commit");
        };
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
        assert!(matches!(
            editor.complete_draft(scene.design_identity).as_slice(),
            [
                EditorEffect::CommitConstruction {
                    expected,
                    proposal: ConstructionProposal::Polyline { .. },
                },
                EditorEffect::ClearConstructionPreview,
            ] if *expected == scene.design_identity
        ));
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
            [EditorEffect::CommitPointMove { expected, point, model_position }, EditorEffect::ClearPointPreview] if *expected == scene.design_identity && *point == points[0] && (model_position[0] - 2.0).abs() < 1.0e-12 && (model_position[1] - 3.0).abs() < 1.0e-12)
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
            [EditorEffect::CommitPointMove { model_position, .. }, EditorEffect::ClearPointPreview] if (model_position[0] - 7.0).abs() < 1.0e-12 && (model_position[1] - 8.0).abs() < 1.0e-12)
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
                        effects.iter().any(|effect| matches!(
                            effect,
                            EditorEffect::CommitConstruction { .. }
                        )),
                        stage + 1 == stages && !explicit_completion
                    );
                }
                let completed = if explicit_completion {
                    editor.complete_draft(scene.design_identity)
                } else {
                    Vec::new()
                };
                assert_eq!(
                    completed
                        .iter()
                        .any(|effect| matches!(effect, EditorEffect::CommitConstruction { .. })),
                    explicit_completion
                );
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
            assert!(
                click(&mut editor, valid)
                    .iter()
                    .any(|effect| matches!(effect, EditorEffect::CommitConstruction { .. }))
            );
        }

        let mut arc = ConstraintEditor::default();
        arc.activate_tool(EditorTool::CounterClockwiseArc);
        click(&mut arc, [0.0, 0.0]);
        assert!(click(&mut arc, [0.0, 0.0]).is_empty());
        click(&mut arc, [2.0, 0.0]);
        assert!(click(&mut arc, [0.0, 0.0]).is_empty());
        assert!(click(&mut arc, [0.0, 2.0]).iter().any(|effect| matches!(
            effect,
            EditorEffect::CommitConstruction {
                proposal: ConstructionProposal::CounterClockwiseArc { .. },
                ..
            }
        )));

        let mut polyline = ConstraintEditor::default();
        polyline.activate_tool(EditorTool::Polyline);
        click(&mut polyline, [0.0, 0.0]);
        assert!(click(&mut polyline, [0.0, 0.0]).is_empty());
        click(&mut polyline, [2.0, 0.0]);
        assert!(
            matches!(polyline.complete_draft(scene.design_identity).as_slice(),
            [EditorEffect::CommitConstruction { proposal: ConstructionProposal::Polyline { points }, .. }, EditorEffect::ClearConstructionPreview] if points.len() == 2)
        );
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
        assert!(matches!(
            editor
                .pointer_down(&scene, pointer(3, second.x, second.y, modifiers))
                .as_slice(),
            [
                EditorEffect::CommitConstruction {
                    proposal: ConstructionProposal::Line { .. },
                    ..
                },
                EditorEffect::ClearConstructionPreview
            ]
        ));
    }
}
