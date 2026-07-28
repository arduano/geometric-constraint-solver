// SPDX-License-Identifier: GPL-3.0-or-later

//! Presentation-independent interaction state for 2D constraint editors.
//!
//! This crate consumes accepted public [`geosolve_sketch`] documents, produces
//! deterministic screen-space scene primitives, resolves pointer hits to persistent
//! sketch identities, and emits typed effects for a host to apply. It owns no solver
//! equations, renderer, DOM integration, persistence, or platform event loop.

mod coordinator;
mod qualification;

pub use coordinator::{
    ActionAvailability, ActionState, AuditDto, AuditProvenance, BranchAction, ContactBranchAction,
    CoordinatorActionKind, CoordinatorError, DisabledReason, EditorMutation, EditorProblemCategory,
    EditorProblemMetadata, EditorProblemScope, EditorProblemTarget, LifecycleDto, LifecycleStatus,
    MeasurementPublication, MutationOutcome, ProblemsDto, ReplayAction, RestoreCheckpoint,
    RetainedEditorCoordinator,
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
    DesignScalarId, DocumentAngleOrientation, DocumentArcSweep, DocumentConstraintDefinition,
    DocumentConstraintId, DocumentCurveSpanRef, DocumentDimensionId, DocumentDimensionMode,
    DocumentEdit, DocumentObjectId, ScalarDomain, ScalarUnit, SketchDesignIdentity, SketchDocument,
    TangentOrientation,
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
}

/// Deterministic presentation-neutral scene derived from one accepted revision.
#[derive(Clone, Debug, PartialEq)]
pub struct EditorScene {
    pub accepted_revision: u64,
    pub design_identity: SketchDesignIdentity,
    pub viewport: Viewport,
    pub points: Vec<ScenePoint>,
    pub curves: Vec<SceneCurve>,
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
                    )?;
                    curves.push(SceneCurve {
                        span,
                        screen_polyline,
                    });
                }
            }
        }
        Ok(Self {
            accepted_revision,
            design_identity,
            viewport,
            points,
            curves,
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
                })
            })
            .min_by(compare_hits);
        if point_hit.is_some() {
            return point_hit;
        }
        self.curves
            .iter()
            .filter_map(|curve| {
                let distance = curve
                    .screen_polyline
                    .windows(2)
                    .map(|segment| point_segment_distance(position, segment[0], segment[1]))
                    .reduce(f64::min)?;
                (distance <= tolerance.curve_pixels).then_some(Hit {
                    item: SelectionItem::Curve(curve.span),
                    distance_pixels: distance,
                })
            })
            .min_by(compare_hits)
    }
}

/// Screen-space picking tolerances. These are interaction policy, not geometry tolerance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PickTolerance {
    pub point_pixels: f64,
    pub curve_pixels: f64,
}

impl Default for PickTolerance {
    fn default() -> Self {
        Self {
            point_pixels: 8.0,
            curve_pixels: 7.0,
        }
    }
}

impl PickTolerance {
    fn is_valid(self) -> bool {
        self.point_pixels.is_finite()
            && self.point_pixels >= 0.0
            && self.curve_pixels.is_finite()
            && self.curve_pixels >= 0.0
    }
}

/// Result of a deterministic scene hit test.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub item: SelectionItem,
    pub distance_pixels: f64,
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
}

/// A typed non-authoritative construction preview.
///
/// Unlike [`ConstructionProposal`], this may describe an incomplete draft and is
/// never committable. Complete previews retain the exact proposal operands that
/// will be emitted on the terminal interaction.
#[derive(Clone, Debug, PartialEq)]
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
}

/// Headless deterministic selection and point-gesture state machine.
#[derive(Clone, Debug)]
pub struct ConstraintEditor {
    selection: Vec<SelectionItem>,
    pick_tolerance: PickTolerance,
    drag_threshold_pixels: f64,
    point_gesture: Option<PointGesture>,
    tool: EditorTool,
    snap_tolerance: SnapTolerance,
    draft: Option<Draft>,
    last_valid_drag_preview: Option<(u64, u64, DesignPointId, [f64; 2])>,
    next_projection_request: u64,
    staged_inference: Option<ProvisionalInferenceCandidate>,
}

impl Default for ConstraintEditor {
    fn default() -> Self {
        Self {
            selection: Vec::new(),
            pick_tolerance: PickTolerance::default(),
            drag_threshold_pixels: 3.0,
            point_gesture: None,
            tool: EditorTool::Select,
            snap_tolerance: SnapTolerance::default(),
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

    #[must_use]
    pub fn selection(&self) -> &[SelectionItem] {
        &self.selection
    }

    /// Replaces ordered persistent selection, removing later duplicates.
    pub fn set_selection(&mut self, selection: impl IntoIterator<Item = SelectionItem>) {
        self.selection.clear();
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
            } else {
                self.selection.push(item);
            }
        } else {
            self.selection.clear();
            self.selection.push(item);
        }
    }

    /// Resolves a pointer press and changes selection immediately.
    pub fn pointer_down(&mut self, scene: &EditorScene, input: PointerInput) -> Vec<EditorEffect> {
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
        let hit = scene.hit_test(input.position, self.pick_tolerance);
        let before = self.selection.clone();
        if let Some(hit) = hit {
            self.select_item(hit.item, input.modifiers);
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
        vec![EditorEffect::RequestProjectedPointMove {
            pointer_id: input.pointer_id,
            request_id,
            point: gesture.point,
            model_position: scene.viewport.screen_to_model(input.position),
        }]
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

    /// Completes a polyline draft. Other tools have no explicit completion action.
    pub fn complete_draft(&mut self, expected: SketchDesignIdentity) -> Vec<EditorEffect> {
        let Some(draft) = self.draft.take() else {
            return Vec::new();
        };
        let proposal = (draft.tool == EditorTool::Polyline)
            .then(|| polyline_proposal(&draft))
            .flatten();
        proposal
            .map(|proposal| commit_construction(expected, proposal))
            .unwrap_or_default()
    }

    /// Whether the current retained draft can be completed by an explicit Finish action.
    #[must_use]
    pub fn can_complete_draft(&self) -> bool {
        self.draft.as_ref().is_some_and(|draft| {
            draft.tool == EditorTool::Polyline && polyline_proposal(draft).is_some()
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
        ) && proposal.is_none()
            || draft.tool == EditorTool::Polyline;
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
    pub kind: ConstraintKind,
    pub label: String,
    pub contacts: Vec<ContactActionChoice>,
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
}

/// Headless editor validation or public scene-evaluation failure.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EditorError {
    #[error("viewport must have finite positive dimensions and scale")]
    InvalidViewport,
    #[error("interaction tolerances must be finite and non-negative")]
    InvalidTolerance,
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
        )?;
    } else {
        output.push(end);
    }
    Ok(())
}

fn compare_hits(first: &Hit, second: &Hit) -> Ordering {
    first
        .distance_pixels
        .total_cmp(&second.distance_pixels)
        .then_with(|| first.item.cmp(&second.item))
}

fn point_segment_distance(point: ScreenPoint, start: ScreenPoint, end: ScreenPoint) -> f64 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length_squared = dx.mul_add(dx, dy * dy);
    if length_squared == 0.0 {
        return point.distance(start);
    }
    let projection = ((point.x - start.x).mul_add(dx, (point.y - start.y) * dy) / length_squared)
        .clamp(0.0, 1.0);
    point.distance(ScreenPoint {
        x: dx.mul_add(projection, start.x),
        y: dy.mul_add(projection, start.y),
    })
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

fn valid_draft_stage(draft: &Draft) -> bool {
    match draft.tool {
        EditorTool::Point => draft.positions.len() == 1,
        EditorTool::Line | EditorTool::Rectangle | EditorTool::Circle => {
            draft.positions.len() < 2 || draft_proposal(draft).is_some()
        }
        EditorTool::Polyline => draft.positions.windows(2).all(nonzero_segment),
        EditorTool::CounterClockwiseArc => {
            let start_is_valid =
                draft.positions.len() < 2 || nonzero_segment(&draft.positions[..2]);
            start_is_valid && (draft.positions.len() < 3 || draft_proposal(draft).is_some())
        }
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
    };
    Some(ConstructionPreview::Complete { proposal, geometry })
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

fn constraint_edit(
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
