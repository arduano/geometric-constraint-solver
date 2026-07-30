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
mod qualification;

pub use annotations::{
    SceneAnnotation, SceneAnnotationGeometry, SceneAnnotationKind, SceneAnnotationVisibility,
    SceneConstraintGlyph, SceneGlyphMarker,
};
pub use authoring::{
    AuthoringApplication, AuthoringOperand, AuthoringOperandKind, AuthoringOptions,
    AuthoringOutcome, AuthoringState, AuthoringTool, AuthoringWarning,
};
pub use coordinator::{
    ALTERNATE_BRANCH_MAX_SEEDS, ActionAvailability, ActionState, AlternateBranchProposal,
    AlternateBranchSearchEvidence, AlternateBranchSearchResult, AlternateBranchSearchStatus,
    AuditDto, AuditProvenance, AuthoringMutation, BranchAction, ContactBranchAction,
    CoordinatorActionKind, CoordinatorError, DimensionTargetDisplayUnit, DimensionTargetMetadata,
    DisabledReason, DisplayDimensionTarget, EditorMutation, EditorProblemCategory,
    EditorProblemMetadata, EditorProblemScope, EditorProblemTarget, LifecycleDto, LifecycleStatus,
    MeasurementPublication, MutationOutcome, ProblemsDto, ProjectedDragRejectionStage,
    ProjectedDragWorkEvidence, ReplayAction, RestoreCheckpoint, RetainedEditorCoordinator,
    display_dimension_target,
};
pub use geosolve_sketch::SketchAcceptedDocumentRedundancy;
#[doc(hidden)]
pub use qualification::{
    M40QualificationCaseResult, M40QualificationReport, m40_qualification_corpus,
    run_m40_qualification, validate_m40_qualification_matrix,
};

use std::cmp::Ordering;

use geosolve_sketch::{
    ContactDomain, ContactNeighborhood, CurveDefinition, CurveId, CurveSpan, DesignPointId,
    DesignScalarId, DocumentAngleOrientation, DocumentArcSweep, DocumentBSplineForm,
    DocumentConstraintDefinition, DocumentConstraintId, DocumentCurveContinuity,
    DocumentCurveCurvatureRelation, DocumentCurveSpanRef, DocumentDimensionId,
    DocumentDimensionMode, DocumentEdit, DocumentHyperbolaBranch, DocumentObjectId,
    MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT, ScalarDomain, ScalarUnit, SketchDesignIdentity,
    SketchDocument, TangentOrientation,
};
use thiserror::Error;

const MAX_TESSELLATION_DEPTH: u8 = 16;

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
}

impl SelectionItem {
    /// Returns the owning persistent document object.
    #[must_use]
    pub const fn object(self) -> DocumentObjectId {
        match self {
            Self::Point(id) => DocumentObjectId::Point(id),
            Self::Curve(span) => DocumentObjectId::Curve(span.curve),
            Self::Constraint(id) => DocumentObjectId::Constraint(id),
            Self::Dimension(id) => DocumentObjectId::Dimension(id),
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
}

/// Deterministic presentation-neutral scene derived from one accepted revision.
#[derive(Clone, Debug, PartialEq)]
pub struct EditorScene {
    pub accepted_revision: u64,
    pub design_identity: SketchDesignIdentity,
    pub viewport: Viewport,
    pub points: Vec<ScenePoint>,
    pub curves: Vec<SceneCurve>,
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
            annotations,
            construction_snap_points,
        })
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
            .filter_map(|point| {
                let distance = position.distance(point.screen_position);
                (distance <= tolerance.point_pixels).then_some(Hit {
                    item: SelectionItem::Point(point.id),
                    distance_pixels: distance,
                    curve_parameter: None,
                })
            })
            .min_by(compare_hits);
        if point_hit.is_some() {
            return point_hit;
        }
        self.curves
            .iter()
            .filter_map(|curve| {
                let (distance, parameter) = curve
                    .screen_polyline
                    .windows(2)
                    .zip(curve.screen_parameters.windows(2))
                    .map(|(segment, parameters)| {
                        let (distance, projection) =
                            point_segment_projection(position, segment[0], segment[1]);
                        (
                            distance,
                            (parameters[1] - parameters[0]).mul_add(projection, parameters[0]),
                        )
                    })
                    .min_by(|first, second| first.0.total_cmp(&second.0))?;
                (distance <= tolerance.curve_pixels).then_some(Hit {
                    item: SelectionItem::Curve(curve.span),
                    distance_pixels: distance,
                    curve_parameter: Some(parameter),
                })
            })
            .min_by(compare_hits)
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
            curve_pixels: 7.0,
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
    pointer_id: u64,
    point: DesignPointId,
    origin: ScreenPoint,
    moved: bool,
    latest_request: Option<u64>,
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
    tool: EditorTool,
    snap_tolerance: SnapTolerance,
    conic_options: ConicConstructionOptions,
    nurbs_options: NurbsConstructionOptions,
    draft: Option<Draft>,
    last_valid_drag_preview: Option<(u64, u64, DesignPointId, [f64; 2])>,
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
            tool: EditorTool::Select,
            snap_tolerance: SnapTolerance::default(),
            conic_options: ConicConstructionOptions::default(),
            nurbs_options: NurbsConstructionOptions::default(),
            draft: None,
            last_valid_drag_preview: None,
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
    /// A [`EditorEffect::ClearPointPreview`] is emitted only when this editor has
    /// previously emitted a point-preview effect that the host may still display.
    pub fn activate_tool(&mut self, tool: EditorTool) -> Vec<EditorEffect> {
        self.tool = tool;
        let mut effects = self.cancel_draft();
        effects.extend(self.cancel_point_gesture());
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
        self.selection.clear();
        self.curve_pick_parameters.clear();
        for item in selection {
            if !self.selection.contains(&item) {
                self.selection.push(item);
            }
        }
    }

    /// Applies one toolkit-independent selection click.
    pub fn select_item(&mut self, item: SelectionItem, modifiers: Modifiers) {
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
        if self.tool != EditorTool::Select {
            return self.draft_down(scene, input);
        }
        let mut effects = Vec::new();
        if self
            .point_gesture
            .is_some_and(|gesture| gesture.pointer_id != input.pointer_id)
        {
            effects.extend(self.cancel_point_gesture());
        }
        let hit = scene
            .annotation_hit_test(
                input.position,
                self.pick_tolerance,
                &self.selection,
                self.visibility_context(),
                problem_items,
            )
            .or_else(|| scene.hit_test(input.position, self.pick_tolerance));
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
            if let SelectionItem::Point(point) = hit.item
                && self.selection.contains(&hit.item)
            {
                self.point_gesture = Some(PointGesture {
                    pointer_id: input.pointer_id,
                    point,
                    origin: input.position,
                    moved: false,
                    latest_request: None,
                });
                self.last_valid_drag_preview = None;
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
        if self.tool != EditorTool::Select {
            return self.draft_move(scene, input);
        }
        let Some(mut gesture) = self.point_gesture else {
            if !input.position.is_finite() {
                return Vec::new();
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
            return self.set_hover_state(target, context);
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
        vec![EditorEffect::RequestProjectedPointMove {
            pointer_id: input.pointer_id,
            request_id,
            point: gesture.point,
            model_position: scene.viewport.screen_to_model(input.position),
        }]
    }

    /// Clears pointer hover when a presentation surface is left.
    pub fn pointer_leave(&mut self) -> Vec<EditorEffect> {
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

    /// Completes an active point gesture. A click emits no geometry edit.
    pub fn pointer_up(
        &mut self,
        _scene: &EditorScene,
        expected: SketchDesignIdentity,
        input: PointerInput,
    ) -> Vec<EditorEffect> {
        if self.tool != EditorTool::Select {
            return Vec::new();
        }
        let Some(gesture) = self.point_gesture else {
            return Vec::new();
        };
        if gesture.pointer_id != input.pointer_id || !input.position.is_finite() {
            return Vec::new();
        }
        self.point_gesture = None;
        if gesture.moved {
            let preview = self
                .last_valid_drag_preview
                .take()
                .filter(|(_, pointer, point, _)| {
                    *pointer == input.pointer_id && *point == gesture.point
                });
            let Some((_, _, _, position)) = preview else {
                return Vec::new();
            };
            vec![
                EditorEffect::CommitPointMove {
                    expected,
                    point: gesture.point,
                    model_position: position,
                },
                EditorEffect::ClearPointPreview,
            ]
        } else {
            Vec::new()
        }
    }

    /// Cancels an active point gesture without a document edit.
    pub fn cancel(&mut self) -> Vec<EditorEffect> {
        let mut effects = self.cancel_draft();
        effects.extend(self.cancel_point_gesture());
        effects.extend(self.set_hover_state(None, None));
        effects
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
        self.last_valid_drag_preview = Some((request_id, pointer_id, point, position));
        vec![EditorEffect::PreviewPointMove {
            point,
            model_position: position,
        }]
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
        self.point_gesture = None;
        self.last_valid_drag_preview
            .take()
            .map(|_| EditorEffect::ClearPointPreview)
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
    #[error(transparent)]
    Document(#[from] geosolve_sketch::DocumentError),
    #[error(transparent)]
    Curve(#[from] geosolve_sketch::DocumentCurveEvaluationError),
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
    if depth < MAX_TESSELLATION_DEPTH && middle.distance(chord_middle) > tolerance {
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

fn compare_hits(first: &Hit, second: &Hit) -> Ordering {
    first
        .distance_pixels
        .total_cmp(&second.distance_pixels)
        .then_with(|| first.item.cmp(&second.item))
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
            for step in 0..=24 {
                let ratio = f64::from(step) / 24.0;
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
        let hit = scene
            .hit_test(ScreenPoint { x: 500.0, y: 306.5 }, PickTolerance::default())
            .expect("line hit within seven pixels");
        assert_eq!(hit.item, SelectionItem::Curve(spans[0]));
        assert!((hit.distance_pixels - 6.5).abs() < 1.0e-12);
        assert_eq!(hit.curve_parameter, Some(0.5));
        assert!(
            scene
                .hit_test(ScreenPoint { x: 500.0, y: 292.0 }, PickTolerance::default(),)
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
            Vec::new()
        );
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
        let mut editor = ConstraintEditor::default();
        editor
            .set_snap_tolerance(SnapTolerance { point_pixels: 8.0 })
            .expect("tolerance");
        editor.activate_tool(EditorTool::Line);
        let endpoint = scene.viewport.model_to_screen([-4.0, 1.0]);
        editor.pointer_down(
            &scene,
            pointer(1, endpoint.x + 8.0, endpoint.y, Modifiers::default()),
        );
        let target = scene.viewport.model_to_screen([0.0, 0.0]);
        let effects =
            editor.pointer_down(&scene, pointer(1, target.x, target.y, Modifiers::default()));
        assert!(
            matches!(effects.as_slice(), [EditorEffect::CommitConstruction { proposal: ConstructionProposal::Line { start: ConstructionPoint::Existing { id, .. }, .. }, .. }, EditorEffect::ClearConstructionPreview] if *id == points[0])
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
            Vec::new()
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
    fn tool_switch_interrupts_drag_and_clears_only_an_existing_preview() {
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
        assert!(editor.activate_tool(EditorTool::Circle).is_empty());
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
