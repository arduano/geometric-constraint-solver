use geosolve_core::CoreError;
use geosolve_geometry::{
    BSplineDefinitionError, BSplineEvaluationError, BSplineSpanIndex, ConicDefinitionError,
    ConicEvaluationError, CurveDifferentialError, NurbsDefinitionError, NurbsEvaluationError,
    Point2,
};
use slotmap::{Key, SlotMap, new_key_type};
use thiserror::Error;

use crate::curves::{
    AngleOrientation, ArcCircleTangencySide, CenterDirectionBranch, CircleTangencyMode,
    LineOffsetOrientation, LineParameterDomain, LineSide,
};

new_key_type! {
    /// Stable identity of a sketch point.
    pub struct PointId;
    /// Stable identity of a line segment.
    pub struct SegmentId;
    /// Stable identity of a circle.
    pub struct CircleId;
    /// Stable identity of a circular arc.
    pub struct ArcId;
    /// Stable identity of a quadratic or cubic Bezier curve.
    pub struct BezierId;
    /// Stable identity of any runtime conic family.
    pub struct ConicId;
    /// Stable identity of a runtime non-rational B-spline.
    pub struct BSplineId;
    /// Stable identity of a runtime NURBS.
    pub struct NurbsId;
    /// Stable identity of a geometric sketch constraint.
    pub struct SketchConstraintId;
    /// Stable identity of a driving or reference dimension.
    pub struct SketchDimensionId;
}

/// Errors produced while editing, compiling, or solving a sketch.
#[derive(Clone, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum SketchError {
    #[error("model scale must be positive and finite, got {0}")]
    InvalidModelScale(f64),
    #[error("{context} must contain only finite coordinates, got ({x}, {y})")]
    NonFinitePoint {
        context: &'static str,
        x: f64,
        y: f64,
    },
    #[error("{context} must be finite, got {value}")]
    NonFiniteValue { context: &'static str, value: f64 },
    #[error("dimension value must be positive and finite, got {0}")]
    InvalidDimensionValue(f64),
    #[error("{0} label must not be empty")]
    EmptyLabel(&'static str),
    #[error("unknown or stale point ID {0:?}")]
    UnknownPoint(PointId),
    #[error("unknown or stale segment ID {0:?}")]
    UnknownSegment(SegmentId),
    #[error("unknown or stale circle ID {0:?}")]
    UnknownCircle(CircleId),
    #[error("unknown or stale arc ID {0:?}")]
    UnknownArc(ArcId),
    #[error("unknown or stale Bezier ID {0:?}")]
    UnknownBezier(BezierId),
    #[error("unknown or stale conic ID {0:?}")]
    UnknownConic(ConicId),
    #[error("unknown or stale B-spline ID {0:?}")]
    UnknownBSpline(BSplineId),
    #[error("unknown or stale NURBS ID {0:?}")]
    UnknownNurbs(NurbsId),
    #[error("unknown or stale sketch constraint ID {0:?}")]
    UnknownConstraint(SketchConstraintId),
    #[error("unknown or stale sketch dimension ID {0:?}")]
    UnknownDimension(SketchDimensionId),
    #[error("point {0:?} is still referenced by sketch geometry")]
    PointInUse(PointId),
    #[error("segment {0:?} is still referenced by a constraint or dimension")]
    SegmentInUse(SegmentId),
    #[error("circle {0:?} is still referenced by a constraint or dimension")]
    CircleInUse(CircleId),
    #[error("arc {0:?} is still referenced by a constraint or dimension")]
    ArcInUse(ArcId),
    #[error("Bezier {0:?} is still referenced by a constraint")]
    BezierInUse(BezierId),
    #[error("conic {0:?} is still referenced by a constraint")]
    ConicInUse(ConicId),
    #[error("B-spline {0:?} is still referenced by a constraint")]
    BSplineInUse(BSplineId),
    #[error("NURBS {0:?} is still referenced by a constraint")]
    NurbsInUse(NurbsId),
    #[error("invalid conic definition: {0}")]
    InvalidConic(ConicDefinitionError),
    #[error("invalid conic evaluation: {0}")]
    InvalidConicEvaluation(ConicEvaluationError),
    #[error("conic {0:?} does not own the requested scalar role")]
    InvalidConicScalarRole(ConicId),
    #[error("conic {0:?} is not a hyperbola segment")]
    InvalidConicBranchRole(ConicId),
    #[error("invalid B-spline definition: {0}")]
    InvalidBSpline(BSplineDefinitionError),
    #[error("invalid B-spline evaluation: {0}")]
    InvalidBSplineEvaluation(BSplineEvaluationError),
    #[error("B-spline control point {0:?} is repeated")]
    RepeatedBSplineControl(PointId),
    #[error("invalid NURBS definition: {0}")]
    InvalidNurbs(NurbsDefinitionError),
    #[error("invalid NURBS evaluation: {0}")]
    InvalidNurbsEvaluation(NurbsEvaluationError),
    #[error("NURBS control point {0:?} is repeated")]
    RepeatedNurbsControl(PointId),
    #[error("NURBS gauge index {gauge_index} is invalid or does not select an exact unit weight")]
    InvalidNurbsGauge { gauge_index: usize },
    #[error("NURBS {nurbs:?} has no weight at index {index}")]
    InvalidNurbsWeightIndex { nurbs: NurbsId, index: usize },
    #[error("NURBS weight {index} must be positive and finite, got {weight}")]
    InvalidNurbsWeight { index: usize, weight: f64 },
    #[error("the selected gauge weight of NURBS {0:?} cannot be edited directly")]
    NurbsGaugeWeightEdit(NurbsId),
    #[error("a line segment requires two different, noncoincident points")]
    DegenerateSegment,
    #[error("retained segment {0:?} has zero-length or non-finite geometry")]
    InvalidSegmentEntity(SegmentId),
    #[error("a point-pair constraint or dimension requires two different points")]
    RepeatedPoint,
    #[error("a driving distance dimension requires noncoincident point geometry")]
    DegenerateDistance,
    #[error("radius must be positive and finite, got {0}")]
    InvalidRadius(f64),
    #[error("arc angles must be finite and select one nonzero sweep")]
    InvalidArcSweep,
    #[error("contact parameter {parameter} is outside {domain}")]
    ParameterOutOfDomain {
        parameter: f64,
        domain: &'static str,
    },
    #[error("curve contact is invalid or degenerate: {0}")]
    InvalidCurveContact(&'static str),
    #[error("invalid curve differential measurement: {0}")]
    InvalidCurveDifferential(CurveDifferentialError),
    #[error("parametric continuity rates must be positive and finite")]
    InvalidContinuityRate,
    #[error("endpoint continuity requires Start or End contact neighborhoods")]
    InvalidContinuityEndpoint,
    #[error("a direction branch requires a finite nonzero direction")]
    InvalidDirectionBranch,
    #[error("internal tangency requires a positive containing-radius difference")]
    InvalidInternalTangency,
    #[error("an oriented angle target must be positive and finite, got {0}")]
    InvalidAngle(f64),
    #[error("the requested entity combination repeats the same entity")]
    RepeatedEntity,
    #[error("constraint {0:?} has no editable contact state")]
    NoContactState(SketchConstraintId),
    #[error("constraint {0:?} is not a circle-circle tangency")]
    NotCircleTangency(SketchConstraintId),
    #[error("circle-arc tangency requires distinct, noncoincident centers")]
    AmbiguousArcCircleTangencyCenters,
    #[error("circle-arc tangency has zero radius derived from its center distance")]
    ZeroDerivedCircleRadius,
    #[error("current centers are incompatible with circle-arc tangency side {0:?}")]
    ArcCircleTangencySideMismatch(ArcCircleTangencySide),
    #[error("constraint {0:?} is not a circle-arc tangency")]
    NotCircleArcTangency(SketchConstraintId),
    #[error(transparent)]
    Core(#[from] CoreError),
}

#[derive(Clone, Debug)]
pub(crate) struct StableStore<K: Key, V> {
    values: SlotMap<K, V>,
    insertion_order: Vec<K>,
}

impl<K: Key, V> StableStore<K, V> {
    fn new() -> Self {
        Self {
            values: SlotMap::with_key(),
            insertion_order: Vec::new(),
        }
    }

    pub(crate) fn insert(&mut self, value: V) -> K {
        let id = self.values.insert(value);
        self.insertion_order.push(id);
        id
    }

    pub(crate) fn get(&self, id: K) -> Option<&V> {
        self.values.get(id)
    }

    pub(crate) fn get_mut(&mut self, id: K) -> Option<&mut V> {
        self.values.get_mut(id)
    }

    pub(crate) fn remove(&mut self, id: K) -> Option<V> {
        self.values.remove(id)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (K, &V)> {
        self.insertion_order
            .iter()
            .filter_map(|&id| self.values.get(id).map(|value| (id, value)))
    }

    pub(crate) fn next_ordinal(&self) -> usize {
        self.insertion_order.len() + 1
    }
}

/// A sketch point with a deterministic human-readable label.
#[derive(Clone, Debug, PartialEq)]
pub struct SketchPoint {
    position: Point2<f64>,
    label: String,
}

impl SketchPoint {
    #[must_use]
    pub fn position(&self) -> Point2<f64> {
        self.position
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// Explicit directed branch retained by a line segment across solves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SegmentBranch {
    reference_direction: [f64; 2],
}

impl SegmentBranch {
    /// Creates an explicit directed branch from a finite nonzero direction.
    ///
    /// # Errors
    ///
    /// Returns [`SketchError::InvalidDirectionBranch`] for a zero or non-finite direction.
    pub fn new(reference_direction: [f64; 2]) -> Result<Self, SketchError> {
        let norm = reference_direction[0].hypot(reference_direction[1]);
        if !norm.is_finite() || norm == 0.0 {
            return Err(SketchError::InvalidDirectionBranch);
        }
        Ok(Self {
            reference_direction: [reference_direction[0] / norm, reference_direction[1] / norm],
        })
    }

    pub(crate) fn from_points(start: Point2<f64>, end: Point2<f64>) -> Result<Self, SketchError> {
        let direction = end - start;
        Self::new([direction.x, direction.y]).map_err(|_| SketchError::DegenerateSegment)
    }

    /// Unit direction selected when this branch was explicitly established.
    #[must_use]
    pub const fn reference_direction(self) -> [f64; 2] {
        self.reference_direction
    }

    /// Whether a directed start/end pair remains in the selected half-plane.
    #[must_use]
    pub fn is_preserved(self, start: Point2<f64>, end: Point2<f64>) -> bool {
        let direction = end - start;
        let projection =
            direction.x * self.reference_direction[0] + direction.y * self.reference_direction[1];
        projection.is_finite() && projection > 0.0
    }
}

/// A directed line segment and its explicit branch state.
#[derive(Clone, Debug, PartialEq)]
pub struct LineSegment {
    start: PointId,
    end: PointId,
    branch: SegmentBranch,
    label: String,
}

impl LineSegment {
    #[must_use]
    pub const fn start(&self) -> PointId {
        self.start
    }

    #[must_use]
    pub const fn end(&self) -> PointId {
        self.end
    }

    #[must_use]
    pub const fn branch(&self) -> SegmentBranch {
        self.branch
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// Cartesian coordinate selected by a fixed-coordinate constraint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoordinateAxis {
    X,
    Y,
}

/// Endpoint selected on a directed segment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SegmentEndpoint {
    Start,
    End,
}

/// Start or end angle coordinate of a directed circular arc.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArcAngleEndpoint {
    Start,
    End,
}

/// Scalar shape coordinate owned by one runtime conic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConicScalarRole {
    MinorAxisRatio,
    MiddleWeight,
    SemiConjugate,
}

/// Closed runtime scalar-property operand used by semantic scalar relations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SketchScalarRef {
    CircleRadius(CircleId),
    ArcRadius(ArcId),
    ArcAngle {
        arc: ArcId,
        endpoint: ArcAngleEndpoint,
    },
    ConicScalar {
        conic: ConicId,
        role: ConicScalarRole,
    },
    NurbsWeight {
        nurbs: NurbsId,
        control_index: usize,
    },
}

/// Relative tangent direction selected by a generic curve tangency.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurveTangentOrientation {
    Aligned,
    Opposed,
}

/// Directed normal side relative to a curve's increasing parameter tangent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurveNormalSide {
    Left,
    Right,
}

/// A line direction constrained against one curve differential location.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurveDirectionRelation {
    Tangent(CurveTangentOrientation),
    Normal(CurveNormalSide),
}

/// Smooth signed equation selected for equal-curvature behavior.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurveCurvatureRelation {
    Signed,
    MagnitudeSameSign,
    MagnitudeOppositeSign,
}

/// Ordered endpoint continuity requested between an incoming and outgoing curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CurveContinuity {
    G0,
    G1,
    G2,
    ParametricC2 { first_rate: f64, second_rate: f64 },
}

/// Equation-free differential measurement requested at one curve contact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurveMeasurementKind {
    SignedCurvature,
    UnsignedCurvature,
    OsculatingRadius,
}

/// Explicit correspondence between the two parent contacts and arc traversal endpoints.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FilletEndpointOrder {
    FirstThenSecond,
    SecondThenFirst,
}

/// Closed runtime curve reference used by geometry-generic contact constraints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SketchCurve {
    Line {
        segment: SegmentId,
        domain: LineParameterDomain,
    },
    Circle(CircleId),
    Arc(ArcId),
    Bezier(BezierId),
    Conic(ConicId),
    BSpline {
        spline: BSplineId,
        span: BSplineSpanIndex,
    },
    Nurbs {
        nurbs: NurbsId,
        span: BSplineSpanIndex,
    },
}

/// Explicit selected endpoint, interior, or finite local neighborhood for a runtime curve contact.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CurveContactNeighborhood {
    Interior,
    Local { lower: f64, upper: f64 },
    Start,
    End,
}

/// One parameterized runtime curve location with explicit neighborhood state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SketchCurveContact {
    pub curve: SketchCurve,
    pub parameter: f64,
    pub neighborhood: CurveContactNeighborhood,
}

/// Supported first-slice geometric constraints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SketchConstraintKind {
    FixedPoint {
        point: PointId,
        target: Point2<f64>,
    },
    /// Native point against an immutable external coefficient.
    ExternalPoint {
        point: PointId,
        target: Point2<f64>,
        provenance: ExternalConstraintProvenance,
    },
    FixedCoordinate {
        point: PointId,
        axis: CoordinateAxis,
        target: f64,
    },
    Coincident {
        first: PointId,
        second: PointId,
    },
    Horizontal {
        segment: SegmentId,
    },
    Vertical {
        segment: SegmentId,
    },
    HorizontalPoints {
        first: PointId,
        second: PointId,
    },
    VerticalPoints {
        first: PointId,
        second: PointId,
    },
    PointOnLine {
        point: PointId,
        segment: SegmentId,
        domain: LineParameterDomain,
        parameter: f64,
    },
    PointOnCircle {
        point: PointId,
        circle: CircleId,
        angle: f64,
    },
    PointOnArc {
        point: PointId,
        arc: ArcId,
        span_parameter: f64,
    },
    PointOnBezier {
        point: PointId,
        bezier: BezierId,
        parameter: f64,
    },
    PointOnCurve {
        point: PointId,
        contact: SketchCurveContact,
    },
    Parallel {
        first: SegmentId,
        second: SegmentId,
    },
    Collinear {
        first: SegmentId,
        second: SegmentId,
    },
    /// Native line support against one immutable directed external support.
    ExternalLineCollinear {
        segment: SegmentId,
        external_start: Point2<f64>,
        external_end: Point2<f64>,
        provenance: ExternalConstraintProvenance,
    },
    Perpendicular {
        first: SegmentId,
        second: SegmentId,
    },
    EqualSegmentLength {
        first: SegmentId,
        second: SegmentId,
    },
    EqualCircleRadius {
        first: CircleId,
        second: CircleId,
    },
    EqualCircleArcRadius {
        circle: CircleId,
        arc: ArcId,
    },
    EqualArcRadius {
        first: ArcId,
        second: ArcId,
    },
    FixedArcAngle {
        arc: ArcId,
        endpoint: ArcAngleEndpoint,
        target: f64,
    },
    FixedScalar {
        property: SketchScalarRef,
        target: f64,
        residual_scale: f64,
    },
    EqualScalar {
        first: SketchScalarRef,
        second: SketchScalarRef,
        residual_scale: f64,
    },
    EqualDistance {
        first: [PointId; 2],
        second: [PointId; 2],
    },
    EqualAngle {
        first: [SegmentId; 2],
        second: [SegmentId; 2],
        first_orientation: AngleOrientation,
        first_winding: i32,
        second_orientation: AngleOrientation,
        second_winding: i32,
    },
    Midpoint {
        point: PointId,
        segment: SegmentId,
    },
    SymmetricAboutLine {
        first: PointId,
        second: PointId,
        line: SegmentId,
    },
    PointSymmetry {
        first: PointId,
        second: PointId,
        center: PointId,
    },
    LineCircleTangency {
        line: SegmentId,
        circle: CircleId,
        domain: LineParameterDomain,
        side: LineSide,
        line_parameter: f64,
        circle_angle: f64,
    },
    CircleCircleTangency {
        first: CircleId,
        second: CircleId,
        mode: CircleTangencyMode,
        center_direction: CenterDirectionBranch,
    },
    CircleArcTangency {
        circle: CircleId,
        arc: ArcId,
        side: ArcCircleTangencySide,
        arc_span_parameter: f64,
        circle_angle: f64,
    },
    LineBezierTangency {
        line: SegmentId,
        endpoint: SegmentEndpoint,
        bezier: BezierId,
        bezier_parameter: f64,
        orientation: CurveTangentOrientation,
    },
    LineCurveTangency {
        line: SegmentId,
        endpoint: SegmentEndpoint,
        contact: SketchCurveContact,
        orientation: CurveTangentOrientation,
    },
    CurveCurveContact {
        first: SketchCurveContact,
        second: SketchCurveContact,
    },
    CurveCurveTangency {
        first: SketchCurveContact,
        second: SketchCurveContact,
        orientation: CurveTangentOrientation,
    },
    CurveDirection {
        line: SegmentId,
        contact: SketchCurveContact,
        relation: CurveDirectionRelation,
    },
    EqualCurvature {
        first: SketchCurveContact,
        second: SketchCurveContact,
        relation: CurveCurvatureRelation,
    },
    EndpointContinuity {
        first: SketchCurveContact,
        second: SketchCurveContact,
        kind: CurveContinuity,
    },
    CurveCurveFillet {
        arc: ArcId,
        first: SketchCurveContact,
        first_side: CurveNormalSide,
        second: SketchCurveContact,
        second_side: CurveNormalSide,
        endpoint_order: FilletEndpointOrder,
    },
}

/// Exact immutable external-input evidence attached to one runtime source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExternalConstraintProvenance {
    pub binding: crate::DocumentExternalBindingId,
    pub expected_kind: crate::ExternalFeatureKindV1,
    pub actual_kind: crate::ExternalFeatureKindV1,
    pub feature_scale: f64,
    pub line_domain: Option<[f64; 2]>,
    pub line_orientation: Option<crate::ExternalLineOrientationV1>,
    pub line_topology_digest: Option<crate::ExternalTopologyDigest>,
    pub set_revision: u64,
    pub set_digest: crate::ExternalSnapshotSetDigest,
    pub source_revision: u64,
    pub source_digest: crate::ExternalSnapshotDigest,
}

/// One stable high-level geometric constraint.
#[derive(Clone, Debug, PartialEq)]
pub struct SketchConstraint {
    pub(crate) kind: SketchConstraintKind,
    ordinal: usize,
}

impl SketchConstraint {
    #[must_use]
    pub const fn kind(&self) -> SketchConstraintKind {
        self.kind
    }

    #[must_use]
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }
}

/// Whether a dimension contributes a hard equation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DimensionMode {
    Driving,
    Reference,
}

/// Supported first-slice dimensions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DimensionKind {
    CoordinateDifference {
        first: PointId,
        second: PointId,
        axis: CoordinateAxis,
        target: f64,
    },
    CircularSweep {
        arc: ArcId,
        target: f64,
    },
    CircularArcLength {
        arc: ArcId,
        target: f64,
    },
    ConicProperty {
        conic: ConicId,
        property: M38ConicProperty,
        target: f64,
    },
    PathLength {
        curve: SketchCurve,
        start: f64,
        end: f64,
        target: f64,
    },
    EqualPathLength {
        first: SketchCurve,
        first_start: f64,
        first_end: f64,
        second: SketchCurve,
        second_start: f64,
        second_end: f64,
        target: f64,
    },
    PointDistance {
        first: PointId,
        second: PointId,
        target: f64,
    },
    SegmentLength {
        segment: SegmentId,
        target: f64,
    },
    CircleRadius {
        circle: CircleId,
        target: f64,
    },
    CircleDiameter {
        circle: CircleId,
        target: f64,
    },
    ArcRadius {
        arc: ArcId,
        target: f64,
    },
    ArcDiameter {
        arc: ArcId,
        target: f64,
    },
    OrientedAngle {
        first: SegmentId,
        second: SegmentId,
        target: f64,
        orientation: AngleOrientation,
    },
    SupportingLineOffset {
        source: SegmentId,
        target_segment: SegmentId,
        target: f64,
        side: LineSide,
        orientation: LineOffsetOrientation,
    },
    ExactTranslatedSegmentOffset {
        source: SegmentId,
        target_segment: SegmentId,
        target: f64,
        side: LineSide,
        orientation: LineOffsetOrientation,
    },
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum M38ConicProperty {
    MajorAxisLength,
    MinorAxisLength,
    LinearEccentricity,
    FocalDistance,
    TransverseAxisLength,
    ConjugateAxisLength,
}

/// One stable driving/reference dimension.
#[derive(Clone, Debug, PartialEq)]
pub struct SketchDimension {
    pub(crate) kind: DimensionKind,
    mode: DimensionMode,
    ordinal: usize,
}

impl SketchDimension {
    #[must_use]
    pub const fn kind(&self) -> DimensionKind {
        self.kind
    }

    #[must_use]
    pub const fn mode(&self) -> DimensionMode {
        self.mode
    }

    #[must_use]
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PersistentSource {
    Constraint(SketchConstraintId),
    Dimension(SketchDimensionId),
}

/// Ordered point/segment sketch domain with guarded edits.
#[derive(Clone, Debug)]
pub struct Sketch {
    pub(crate) model_scale: f64,
    pub(crate) points: StableStore<PointId, SketchPoint>,
    pub(crate) segments: StableStore<SegmentId, LineSegment>,
    pub(crate) circles: StableStore<CircleId, crate::curves::Circle>,
    pub(crate) arcs: StableStore<ArcId, crate::curves::CircularArc>,
    pub(crate) beziers: StableStore<BezierId, crate::beziers::BezierCurve>,
    pub(crate) conics: StableStore<ConicId, crate::conics::ConicCurve>,
    pub(crate) bsplines: StableStore<BSplineId, crate::bsplines::BSplineCurve>,
    pub(crate) nurbs: StableStore<NurbsId, crate::nurbs::NurbsCurve>,
    pub(crate) constraints: StableStore<SketchConstraintId, SketchConstraint>,
    pub(crate) dimensions: StableStore<SketchDimensionId, SketchDimension>,
    pub(crate) source_order: Vec<PersistentSource>,
}

impl Sketch {
    /// Creates an empty sketch with one positive finite characteristic scale.
    ///
    /// # Errors
    ///
    /// Returns [`SketchError::InvalidModelScale`] for a nonpositive or non-finite scale.
    pub fn new(model_scale: f64) -> Result<Self, SketchError> {
        validate_model_scale(model_scale)?;
        Ok(Self {
            model_scale,
            points: StableStore::new(),
            segments: StableStore::new(),
            circles: StableStore::new(),
            arcs: StableStore::new(),
            beziers: StableStore::new(),
            conics: StableStore::new(),
            bsplines: StableStore::new(),
            nurbs: StableStore::new(),
            constraints: StableStore::new(),
            dimensions: StableStore::new(),
            source_order: Vec::new(),
        })
    }

    #[must_use]
    pub const fn model_scale(&self) -> f64 {
        self.model_scale
    }

    /// Replaces the characteristic model scale.
    ///
    /// # Errors
    ///
    /// Returns [`SketchError::InvalidModelScale`] for a nonpositive or non-finite scale.
    pub fn set_model_scale(&mut self, model_scale: f64) -> Result<(), SketchError> {
        validate_model_scale(model_scale)?;
        self.model_scale = model_scale;
        Ok(())
    }

    /// Adds a point with a deterministic generated label.
    ///
    /// # Errors
    ///
    /// Returns an error when either coordinate is non-finite.
    pub fn add_point(&mut self, position: Point2<f64>) -> Result<PointId, SketchError> {
        let label = format!("P{}", self.points.next_ordinal());
        self.add_named_point(label, position)
    }

    /// Adds a point with an explicit audit label.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty label or non-finite coordinate.
    pub fn add_named_point(
        &mut self,
        label: impl Into<String>,
        position: Point2<f64>,
    ) -> Result<PointId, SketchError> {
        validate_point(position, "point position")?;
        let label = nonempty_label(label, "point")?;
        Ok(self.points.insert(SketchPoint { position, label }))
    }

    #[must_use]
    pub fn point(&self, point: PointId) -> Option<&SketchPoint> {
        self.points.get(point)
    }

    pub fn points(&self) -> impl Iterator<Item = (PointId, &SketchPoint)> {
        self.points.iter()
    }

    /// Replaces a point's warm-start position without changing segment branch state.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale ID or non-finite coordinate.
    pub fn set_point_position(
        &mut self,
        point: PointId,
        position: Point2<f64>,
    ) -> Result<(), SketchError> {
        validate_point(position, "point position")?;
        self.points
            .get_mut(point)
            .ok_or(SketchError::UnknownPoint(point))?
            .position = position;
        Ok(())
    }

    /// Removes an unreferenced point.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale ID or a point still referenced by geometry.
    pub fn remove_point(&mut self, point: PointId) -> Result<SketchPoint, SketchError> {
        if self
            .segments
            .iter()
            .any(|(_, segment)| segment.start == point || segment.end == point)
            || self
                .circles
                .iter()
                .any(|(_, circle)| circle.center() == point)
            || self.arcs.iter().any(|(_, arc)| arc.center() == point)
            || self
                .beziers
                .iter()
                .any(|(_, curve)| curve.controls().contains(&point))
            || self
                .conics
                .iter()
                .any(|(_, curve)| curve.kind().references_point(point))
            || self
                .bsplines
                .iter()
                .any(|(_, curve)| curve.controls().contains(&point))
            || self
                .nurbs
                .iter()
                .any(|(_, curve)| curve.controls().contains(&point))
            || self
                .constraints
                .iter()
                .any(|(_, constraint)| constraint_references_point(constraint.kind, point, self))
            || self
                .dimensions
                .iter()
                .any(|(_, dimension)| dimension_references_point(dimension.kind, point, self))
        {
            return Err(SketchError::PointInUse(point));
        }
        self.points
            .remove(point)
            .ok_or(SketchError::UnknownPoint(point))
    }

    /// Adds a directed segment with a generated label and current-direction branch.
    ///
    /// # Errors
    ///
    /// Returns an error for stale, repeated, or coincident endpoint geometry.
    pub fn add_segment(&mut self, start: PointId, end: PointId) -> Result<SegmentId, SketchError> {
        let label = format!("S{}", self.segments.next_ordinal());
        self.add_named_segment(label, start, end)
    }

    /// Adds a named directed segment and records its current-direction branch.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty label or invalid endpoint geometry.
    pub fn add_named_segment(
        &mut self,
        label: impl Into<String>,
        start: PointId,
        end: PointId,
    ) -> Result<SegmentId, SketchError> {
        if start == end {
            return Err(SketchError::DegenerateSegment);
        }
        let start_position = self.point_position(start)?;
        let end_position = self.point_position(end)?;
        let branch = SegmentBranch::from_points(start_position, end_position)?;
        let label = nonempty_label(label, "segment")?;
        Ok(self.segments.insert(LineSegment {
            start,
            end,
            branch,
            label,
        }))
    }

    /// Adds a named directed segment while restoring an explicit saved branch.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid endpoint geometry, an invalid branch, or
    /// current geometry on the opposite side of that branch.
    pub fn add_named_segment_with_branch(
        &mut self,
        label: impl Into<String>,
        start: PointId,
        end: PointId,
        branch: SegmentBranch,
    ) -> Result<SegmentId, SketchError> {
        if start == end {
            return Err(SketchError::DegenerateSegment);
        }
        let start_position = self.point_position(start)?;
        let end_position = self.point_position(end)?;
        SegmentBranch::from_points(start_position, end_position)?;
        if !branch.is_preserved(start_position, end_position) {
            return Err(SketchError::InvalidDirectionBranch);
        }
        let label = nonempty_label(label, "segment")?;
        Ok(self.segments.insert(LineSegment {
            start,
            end,
            branch,
            label,
        }))
    }

    #[must_use]
    pub fn segment(&self, segment: SegmentId) -> Option<&LineSegment> {
        self.segments.get(segment)
    }

    pub fn segments(&self) -> impl Iterator<Item = (SegmentId, &LineSegment)> {
        self.segments.iter()
    }

    /// Reports whether the current geometry is on a segment's explicit branch.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale segment or endpoint ID.
    pub fn segment_branch_is_preserved(&self, segment: SegmentId) -> Result<bool, SketchError> {
        let segment = self
            .segments
            .get(segment)
            .ok_or(SketchError::UnknownSegment(segment))?;
        Ok(segment.branch.is_preserved(
            self.point_position(segment.start)?,
            self.point_position(segment.end)?,
        ))
    }

    /// Reports whether this segment currently has a discrete axis/length root.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale segment ID.
    pub fn segment_has_enforced_branch(&self, segment: SegmentId) -> Result<bool, SketchError> {
        self.segments
            .get(segment)
            .ok_or(SketchError::UnknownSegment(segment))?;
        Ok(self.segment_branch_is_enforced(segment))
    }

    pub(crate) fn segment_branch_is_enforced(&self, segment: SegmentId) -> bool {
        let has_axis_constraint = self.constraints.iter().any(|(_, constraint)| {
            matches!(
                constraint.kind,
                SketchConstraintKind::Horizontal { segment: id }
                    | SketchConstraintKind::Vertical { segment: id }
                    if id == segment
            )
        });
        let has_driving_length = self.dimensions.iter().any(|(_, dimension)| {
            dimension.mode == DimensionMode::Driving
                && matches!(
                    dimension.kind,
                    DimensionKind::SegmentLength { segment: id, .. } if id == segment
                )
        });
        let has_sided_fillet = self.constraints.iter().any(|(_, constraint)| {
            matches!(
                constraint.kind,
                SketchConstraintKind::CurveCurveFillet { first, second, .. }
                    if first.curve.references_segment(segment)
                        || second.curve.references_segment(segment)
            )
        });
        has_sided_fillet || (has_axis_constraint && has_driving_length)
    }

    /// Explicitly selects the segment branch represented by its current direction.
    ///
    /// # Errors
    ///
    /// Returns an error for stale IDs or coincident endpoints.
    pub fn reselect_segment_branch(&mut self, segment: SegmentId) -> Result<(), SketchError> {
        let (start, end) = self.segment_endpoints(segment)?;
        let branch =
            SegmentBranch::from_points(self.point_position(start)?, self.point_position(end)?)?;
        self.segments
            .get_mut(segment)
            .ok_or(SketchError::UnknownSegment(segment))?
            .branch = branch;
        Ok(())
    }

    /// Removes a segment not referenced by a constraint or dimension.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale or referenced segment.
    pub fn remove_segment(&mut self, segment: SegmentId) -> Result<LineSegment, SketchError> {
        if self
            .constraints
            .iter()
            .any(|(_, constraint)| constraint_references_segment(constraint.kind, segment))
            || self
                .dimensions
                .iter()
                .any(|(_, dimension)| dimension_references_segment(dimension.kind, segment))
        {
            return Err(SketchError::SegmentInUse(segment));
        }
        self.segments
            .remove(segment)
            .ok_or(SketchError::UnknownSegment(segment))
    }

    /// Fixes a point at its current position using trusted core elimination.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale point ID.
    pub fn add_fixed_point(&mut self, point: PointId) -> Result<SketchConstraintId, SketchError> {
        let target = self.point_position(point)?;
        Ok(self.insert_constraint(SketchConstraintKind::FixedPoint { point, target }))
    }

    /// Fixes a point at an explicit finite target using trusted core elimination.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale point ID or non-finite target.
    pub fn add_fixed_point_at(
        &mut self,
        point: PointId,
        target: Point2<f64>,
    ) -> Result<SketchConstraintId, SketchError> {
        self.point_position(point)?;
        validate_point(target, "fixed-point target")?;
        Ok(self.insert_constraint(SketchConstraintKind::FixedPoint { point, target }))
    }

    /// Adds a scalar fixed-coordinate equation.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale point ID or non-finite target.
    pub fn add_fixed_coordinate(
        &mut self,
        point: PointId,
        axis: CoordinateAxis,
        target: f64,
    ) -> Result<SketchConstraintId, SketchError> {
        self.point_position(point)?;
        validate_finite(target, "fixed-coordinate target")?;
        Ok(
            self.insert_constraint(SketchConstraintKind::FixedCoordinate {
                point,
                axis,
                target,
            }),
        )
    }

    /// Adds a two-row point coincidence constraint.
    ///
    /// # Errors
    ///
    /// Returns an error for stale or repeated point IDs.
    pub fn add_coincident(
        &mut self,
        first: PointId,
        second: PointId,
    ) -> Result<SketchConstraintId, SketchError> {
        self.validate_point_pair(first, second)?;
        Ok(self.insert_constraint(SketchConstraintKind::Coincident { first, second }))
    }

    /// Constrains a segment to be horizontal.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale segment ID.
    pub fn add_horizontal(
        &mut self,
        segment: SegmentId,
    ) -> Result<SketchConstraintId, SketchError> {
        self.validate_segment_geometry(segment)?;
        Ok(self.insert_constraint(SketchConstraintKind::Horizontal { segment }))
    }

    /// Constrains a segment to be vertical.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale segment ID.
    pub fn add_vertical(&mut self, segment: SegmentId) -> Result<SketchConstraintId, SketchError> {
        self.validate_segment_geometry(segment)?;
        Ok(self.insert_constraint(SketchConstraintKind::Vertical { segment }))
    }

    /// Constrains two arbitrary stored point features to share their Y coordinate.
    ///
    /// # Errors
    /// Returns an error for missing or repeated point identities.
    pub fn add_horizontal_points(
        &mut self,
        first: PointId,
        second: PointId,
    ) -> Result<SketchConstraintId, SketchError> {
        self.validate_point_pair(first, second)?;
        Ok(self.insert_constraint(SketchConstraintKind::HorizontalPoints { first, second }))
    }

    /// Constrains two arbitrary stored point features to share their X coordinate.
    ///
    /// # Errors
    /// Returns an error for missing or repeated point identities.
    pub fn add_vertical_points(
        &mut self,
        first: PointId,
        second: PointId,
    ) -> Result<SketchConstraintId, SketchError> {
        self.validate_point_pair(first, second)?;
        Ok(self.insert_constraint(SketchConstraintKind::VerticalPoints { first, second }))
    }

    /// Constrains two directed line supports to the same infinite supporting line.
    ///
    /// # Errors
    /// Returns an error for a missing or degenerate support.
    pub fn add_collinear(
        &mut self,
        first: SegmentId,
        second: SegmentId,
    ) -> Result<SketchConstraintId, SketchError> {
        self.validate_segment_geometry(first)?;
        self.validate_segment_geometry(second)?;
        Ok(self.insert_constraint(SketchConstraintKind::Collinear { first, second }))
    }

    /// Constrains a native point to one immutable external point coefficient.
    ///
    /// # Errors
    /// Returns an error for a missing native point or non-finite target.
    pub fn add_external_point(
        &mut self,
        point: PointId,
        target: Point2<f64>,
        provenance: ExternalConstraintProvenance,
    ) -> Result<SketchConstraintId, SketchError> {
        self.point_position(point)?;
        if !target.x.is_finite() || !target.y.is_finite() {
            return Err(SketchError::NonFinitePoint {
                context: "external point coefficient",
                x: target.x,
                y: target.y,
            });
        }
        Ok(self.insert_constraint(SketchConstraintKind::ExternalPoint {
            point,
            target,
            provenance,
        }))
    }

    /// Constrains a native line to one immutable external directed support.
    ///
    /// # Errors
    /// Returns an error for a missing or degenerate native or external support.
    pub fn add_external_line_collinear(
        &mut self,
        segment: SegmentId,
        external_start: Point2<f64>,
        external_end: Point2<f64>,
        provenance: ExternalConstraintProvenance,
    ) -> Result<SketchConstraintId, SketchError> {
        self.validate_segment_geometry(segment)?;
        let dx = external_end.x - external_start.x;
        let dy = external_end.y - external_start.y;
        if !external_start.x.is_finite()
            || !external_start.y.is_finite()
            || !external_end.x.is_finite()
            || !external_end.y.is_finite()
            || !dx.is_finite()
            || !dy.is_finite()
            || dx.hypot(dy) <= f64::EPSILON
        {
            return Err(SketchError::DegenerateSegment);
        }
        Ok(
            self.insert_constraint(SketchConstraintKind::ExternalLineCollinear {
                segment,
                external_start,
                external_end,
                provenance,
            }),
        )
    }

    /// Constrains the midpoint of two points to an explicit center point.
    ///
    /// # Errors
    /// Returns an error for missing or repeated point identities.
    pub fn add_point_symmetry(
        &mut self,
        first: PointId,
        second: PointId,
        center: PointId,
    ) -> Result<SketchConstraintId, SketchError> {
        self.validate_point_pair(first, second)?;
        self.point_position(center)?;
        Ok(self.insert_constraint(SketchConstraintKind::PointSymmetry {
            first,
            second,
            center,
        }))
    }

    /// Equates two point-pair distances without introducing a target dimension.
    ///
    /// # Errors
    /// Returns an error for a missing or repeated point in either pair.
    pub fn add_equal_distance(
        &mut self,
        first_start: PointId,
        first_end: PointId,
        second_start: PointId,
        second_end: PointId,
    ) -> Result<SketchConstraintId, SketchError> {
        self.validate_point_pair(first_start, first_end)?;
        self.validate_point_pair(second_start, second_end)?;
        Ok(self.insert_constraint(SketchConstraintKind::EqualDistance {
            first: [first_start, first_end],
            second: [second_start, second_end],
        }))
    }

    /// Equates two explicitly oriented and unwrapped angles between line supports.
    ///
    /// # Errors
    /// Returns an error for a missing or degenerate support.
    #[allow(clippy::too_many_arguments)]
    pub fn add_equal_angle(
        &mut self,
        first_from: SegmentId,
        first_to: SegmentId,
        second_from: SegmentId,
        second_to: SegmentId,
        first_orientation: AngleOrientation,
        first_winding: i32,
        second_orientation: AngleOrientation,
        second_winding: i32,
    ) -> Result<SketchConstraintId, SketchError> {
        for segment in [first_from, first_to, second_from, second_to] {
            self.validate_segment_geometry(segment)?;
        }
        Ok(self.insert_constraint(SketchConstraintKind::EqualAngle {
            first: [first_from, first_to],
            second: [second_from, second_to],
            first_orientation,
            first_winding,
            second_orientation,
            second_winding,
        }))
    }

    /// Equates a full-circle radius with a circular-arc radius.
    ///
    /// # Errors
    /// Returns an error for a missing circle or arc.
    pub fn add_equal_circle_arc_radius(
        &mut self,
        circle: CircleId,
        arc: ArcId,
    ) -> Result<SketchConstraintId, SketchError> {
        self.circle_value(circle)?;
        self.arc_value(arc)?;
        Ok(self.insert_constraint(SketchConstraintKind::EqualCircleArcRadius { circle, arc }))
    }

    /// Equates the radii of two circular arcs.
    ///
    /// # Errors
    /// Returns an error for a missing arc.
    pub fn add_equal_arc_radius(
        &mut self,
        first: ArcId,
        second: ArcId,
    ) -> Result<SketchConstraintId, SketchError> {
        self.arc_value(first)?;
        self.arc_value(second)?;
        Ok(self.insert_constraint(SketchConstraintKind::EqualArcRadius { first, second }))
    }

    /// Fixes one directed circular-arc endpoint angle as part of grouped entity blocking.
    ///
    /// # Errors
    /// Returns an error for a missing arc or non-finite target.
    pub fn add_fixed_arc_angle(
        &mut self,
        arc: ArcId,
        endpoint: ArcAngleEndpoint,
        target: f64,
    ) -> Result<SketchConstraintId, SketchError> {
        self.arc_value(arc)?;
        validate_finite(target, "fixed arc angle")?;
        Ok(self.insert_constraint(SketchConstraintKind::FixedArcAngle {
            arc,
            endpoint,
            target,
        }))
    }

    /// Fixes one mapped scalar property through an executable hard row.
    ///
    /// # Errors
    /// Returns an error for a missing property or invalid target/scale.
    pub fn add_fixed_scalar(
        &mut self,
        property: SketchScalarRef,
        target: f64,
        residual_scale: f64,
    ) -> Result<SketchConstraintId, SketchError> {
        self.validate_scalar_ref(property)?;
        validate_finite(target, "fixed scalar target")?;
        validate_model_scale(residual_scale)?;
        Ok(self.insert_constraint(SketchConstraintKind::FixedScalar {
            property,
            target,
            residual_scale,
        }))
    }

    /// Equates two mapped scalar properties through one executable hard row.
    ///
    /// # Errors
    /// Returns an error for a missing property or invalid residual scale.
    pub fn add_equal_scalar(
        &mut self,
        first: SketchScalarRef,
        second: SketchScalarRef,
        residual_scale: f64,
    ) -> Result<SketchConstraintId, SketchError> {
        self.validate_scalar_ref(first)?;
        self.validate_scalar_ref(second)?;
        validate_model_scale(residual_scale)?;
        Ok(self.insert_constraint(SketchConstraintKind::EqualScalar {
            first,
            second,
            residual_scale,
        }))
    }

    fn validate_scalar_ref(&self, property: SketchScalarRef) -> Result<(), SketchError> {
        match property {
            SketchScalarRef::CircleRadius(circle) => self.circle_value(circle).map(|_| ()),
            SketchScalarRef::ArcRadius(arc) | SketchScalarRef::ArcAngle { arc, .. } => {
                self.arc_value(arc).map(|_| ())
            }
            SketchScalarRef::ConicScalar { conic, .. } => self
                .conics
                .get(conic)
                .map(|_| ())
                .ok_or(SketchError::UnknownConic(conic)),
            SketchScalarRef::NurbsWeight { nurbs, .. } => self
                .nurbs
                .get(nurbs)
                .map(|_| ())
                .ok_or(SketchError::UnknownNurbs(nurbs)),
        }
    }

    #[must_use]
    pub fn constraint(&self, constraint: SketchConstraintId) -> Option<&SketchConstraint> {
        self.constraints.get(constraint)
    }

    pub fn constraints(&self) -> impl Iterator<Item = (SketchConstraintId, &SketchConstraint)> {
        self.constraints.iter()
    }

    /// Removes a constraint while leaving its stable ID stale.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown or stale constraint ID.
    pub fn remove_constraint(
        &mut self,
        constraint: SketchConstraintId,
    ) -> Result<SketchConstraint, SketchError> {
        self.constraints
            .remove(constraint)
            .ok_or(SketchError::UnknownConstraint(constraint))
    }

    /// Adds a point-distance driving or reference dimension.
    ///
    /// # Errors
    ///
    /// Returns an error for stale/repeated points or an invalid target.
    pub fn add_point_distance(
        &mut self,
        first: PointId,
        second: PointId,
        target: f64,
        mode: DimensionMode,
    ) -> Result<SketchDimensionId, SketchError> {
        self.validate_point_pair(first, second)?;
        validate_dimension_value(target)?;
        if mode == DimensionMode::Driving
            && self.point_position(first)? == self.point_position(second)?
        {
            return Err(SketchError::DegenerateDistance);
        }
        Ok(self.insert_dimension(
            DimensionKind::PointDistance {
                first,
                second,
                target,
            },
            mode,
        ))
    }

    pub(crate) fn add_coordinate_difference_dimension(
        &mut self,
        first: PointId,
        second: PointId,
        axis: CoordinateAxis,
        target: f64,
        mode: DimensionMode,
    ) -> Result<SketchDimensionId, SketchError> {
        self.validate_point_pair(first, second)?;
        validate_finite(target, "coordinate difference target")?;
        Ok(self.insert_dimension(
            DimensionKind::CoordinateDifference {
                first,
                second,
                axis,
                target,
            },
            mode,
        ))
    }

    pub(crate) fn add_circular_sweep_dimension(
        &mut self,
        arc: ArcId,
        target: f64,
    ) -> Result<SketchDimensionId, SketchError> {
        self.arc_value(arc)?;
        validate_finite(target, "circular sweep target")?;
        Ok(self.insert_dimension(
            DimensionKind::CircularSweep { arc, target },
            DimensionMode::Driving,
        ))
    }

    pub(crate) fn add_circular_arc_length_dimension(
        &mut self,
        arc: ArcId,
        target: f64,
    ) -> Result<SketchDimensionId, SketchError> {
        self.arc_value(arc)?;
        validate_dimension_value(target)?;
        Ok(self.insert_dimension(
            DimensionKind::CircularArcLength { arc, target },
            DimensionMode::Driving,
        ))
    }

    pub(crate) fn add_conic_property_dimension(
        &mut self,
        conic: ConicId,
        property: M38ConicProperty,
        target: f64,
    ) -> Result<SketchDimensionId, SketchError> {
        self.conic_value(conic)?;
        validate_dimension_value(target)?;
        Ok(self.insert_dimension(
            DimensionKind::ConicProperty {
                conic,
                property,
                target,
            },
            DimensionMode::Driving,
        ))
    }

    pub(crate) fn add_path_length_dimension(
        &mut self,
        curve: SketchCurve,
        start: f64,
        end: f64,
        target: f64,
    ) -> Result<SketchDimensionId, SketchError> {
        validate_path_interval(start, end)?;
        validate_dimension_value(target)?;
        Ok(self.insert_dimension(
            DimensionKind::PathLength {
                curve,
                start,
                end,
                target,
            },
            DimensionMode::Driving,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn add_equal_path_length_dimension(
        &mut self,
        first: SketchCurve,
        first_start: f64,
        first_end: f64,
        second: SketchCurve,
        second_start: f64,
        second_end: f64,
        target: f64,
    ) -> Result<SketchDimensionId, SketchError> {
        validate_path_interval(first_start, first_end)?;
        validate_path_interval(second_start, second_end)?;
        validate_finite(target, "equal path length target")?;
        Ok(self.insert_dimension(
            DimensionKind::EqualPathLength {
                first,
                first_start,
                first_end,
                second,
                second_start,
                second_end,
                target,
            },
            DimensionMode::Driving,
        ))
    }

    /// Adds a segment-length driving or reference dimension.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale segment or invalid target.
    pub fn add_segment_length(
        &mut self,
        segment: SegmentId,
        target: f64,
        mode: DimensionMode,
    ) -> Result<SketchDimensionId, SketchError> {
        if mode == DimensionMode::Driving {
            self.validate_segment_geometry(segment)?;
        } else {
            self.segment_endpoints(segment)?;
        }
        validate_dimension_value(target)?;
        Ok(self.insert_dimension(DimensionKind::SegmentLength { segment, target }, mode))
    }

    #[must_use]
    pub fn dimension(&self, dimension: SketchDimensionId) -> Option<&SketchDimension> {
        self.dimensions.get(dimension)
    }

    pub fn dimensions(&self) -> impl Iterator<Item = (SketchDimensionId, &SketchDimension)> {
        self.dimensions.iter()
    }

    /// Toggles a dimension without changing its stable ID or source position.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown or stale dimension ID.
    pub fn set_dimension_mode(
        &mut self,
        dimension: SketchDimensionId,
        mode: DimensionMode,
    ) -> Result<(), SketchError> {
        self.dimensions
            .get_mut(dimension)
            .ok_or(SketchError::UnknownDimension(dimension))?
            .mode = mode;
        Ok(())
    }

    /// Edits a finite positive dimension target without changing its stable ID.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid target or stale dimension ID.
    pub fn set_dimension_target(
        &mut self,
        dimension: SketchDimensionId,
        target: f64,
    ) -> Result<(), SketchError> {
        validate_dimension_value(target)?;
        let kind = &mut self
            .dimensions
            .get_mut(dimension)
            .ok_or(SketchError::UnknownDimension(dimension))?
            .kind;
        match kind {
            DimensionKind::CoordinateDifference {
                target: current, ..
            }
            | DimensionKind::CircularSweep {
                target: current, ..
            }
            | DimensionKind::CircularArcLength {
                target: current, ..
            }
            | DimensionKind::ConicProperty {
                target: current, ..
            }
            | DimensionKind::PathLength {
                target: current, ..
            }
            | DimensionKind::EqualPathLength {
                target: current, ..
            }
            | DimensionKind::PointDistance {
                target: current, ..
            }
            | DimensionKind::SegmentLength {
                target: current, ..
            }
            | DimensionKind::CircleRadius {
                target: current, ..
            }
            | DimensionKind::CircleDiameter {
                target: current, ..
            }
            | DimensionKind::ArcRadius {
                target: current, ..
            }
            | DimensionKind::ArcDiameter {
                target: current, ..
            }
            | DimensionKind::OrientedAngle {
                target: current, ..
            }
            | DimensionKind::SupportingLineOffset {
                target: current, ..
            }
            | DimensionKind::ExactTranslatedSegmentOffset {
                target: current, ..
            } => *current = target,
        }
        Ok(())
    }

    /// Removes a dimension while leaving its stable ID stale.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown or stale dimension ID.
    pub fn remove_dimension(
        &mut self,
        dimension: SketchDimensionId,
    ) -> Result<SketchDimension, SketchError> {
        self.dimensions
            .remove(dimension)
            .ok_or(SketchError::UnknownDimension(dimension))
    }

    pub(crate) fn point_position(&self, point: PointId) -> Result<Point2<f64>, SketchError> {
        self.points
            .get(point)
            .map(SketchPoint::position)
            .ok_or(SketchError::UnknownPoint(point))
    }

    pub(crate) fn segment_endpoints(
        &self,
        segment: SegmentId,
    ) -> Result<(PointId, PointId), SketchError> {
        self.segments
            .get(segment)
            .map(|segment| (segment.start, segment.end))
            .ok_or(SketchError::UnknownSegment(segment))
    }

    pub(crate) fn validate_segment_geometry(&self, segment: SegmentId) -> Result<(), SketchError> {
        let (start, end) = self.segment_endpoints(segment)?;
        SegmentBranch::from_points(self.point_position(start)?, self.point_position(end)?)?;
        Ok(())
    }

    pub(crate) fn preflight_segments(&self) -> Result<(), SketchError> {
        for (segment_id, segment) in self.segments.iter() {
            let start = self.point_position(segment.start)?;
            let end = self.point_position(segment.end)?;
            let length = (end - start).norm();
            if !length.is_finite() || length == 0.0 {
                return Err(SketchError::InvalidSegmentEntity(segment_id));
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn dimension_value(&self, dimension: &SketchDimension) -> Result<f64, SketchError> {
        let value = match dimension.kind {
            DimensionKind::CoordinateDifference {
                first,
                second,
                axis,
                ..
            } => {
                let coordinate = match axis {
                    CoordinateAxis::X => 0,
                    CoordinateAxis::Y => 1,
                };
                self.point_position(second)?[coordinate] - self.point_position(first)?[coordinate]
            }
            DimensionKind::CircularSweep { arc, .. } => self.arc_value(arc)?.signed_sweep(),
            DimensionKind::CircularArcLength { arc, .. } => {
                let arc = self.arc_value(arc)?;
                arc.radius() * arc.signed_sweep().abs()
            }
            DimensionKind::ConicProperty {
                conic, property, ..
            } => self.m38_conic_property(conic, property)?,
            DimensionKind::PathLength {
                curve, start, end, ..
            } => self.path_length_value(curve, start, end)?,
            DimensionKind::EqualPathLength {
                first,
                first_start,
                first_end,
                second,
                second_start,
                second_end,
                ..
            } => {
                self.path_length_value(first, first_start, first_end)?
                    - self.path_length_value(second, second_start, second_end)?
            }
            DimensionKind::PointDistance { first, second, .. } => {
                (self.point_position(second)? - self.point_position(first)?).norm()
            }
            DimensionKind::SegmentLength { segment, .. } => {
                let (first, second) = self.segment_endpoints(segment)?;
                (self.point_position(second)? - self.point_position(first)?).norm()
            }
            DimensionKind::CircleRadius { circle, .. } => self
                .circles
                .get(circle)
                .ok_or(SketchError::UnknownCircle(circle))?
                .radius(),
            DimensionKind::CircleDiameter { circle, .. } => {
                2.0 * self
                    .circles
                    .get(circle)
                    .ok_or(SketchError::UnknownCircle(circle))?
                    .radius()
            }
            DimensionKind::ArcRadius { arc, .. } => self
                .arcs
                .get(arc)
                .ok_or(SketchError::UnknownArc(arc))?
                .radius(),
            DimensionKind::ArcDiameter { arc, .. } => {
                2.0 * self
                    .arcs
                    .get(arc)
                    .ok_or(SketchError::UnknownArc(arc))?
                    .radius()
            }
            DimensionKind::OrientedAngle {
                first,
                second,
                target,
                orientation,
            } => self.oriented_angle_value(first, second, orientation, target)?,
            DimensionKind::SupportingLineOffset {
                source,
                target_segment,
                side,
                orientation,
                ..
            }
            | DimensionKind::ExactTranslatedSegmentOffset {
                source,
                target_segment,
                side,
                orientation,
                ..
            } => {
                let (source_start, source_end) = self.segment_endpoints(source)?;
                let (target_start, target_end) = self.segment_endpoints(target_segment)?;
                let (target_start, _) = orientation.target_endpoints(target_start, target_end);
                let source_start = self.point_position(source_start)?;
                let source_direction = self.point_position(source_end)? - source_start;
                let source_length = source_direction.norm();
                let target_start = self.point_position(target_start)?;
                side.sign()
                    * (source_direction.x * (target_start.y - source_start.y)
                        - source_direction.y * (target_start.x - source_start.x))
                    / source_length
            }
        };
        validate_finite(value, "reference dimension value")?;
        Ok(value)
    }

    fn m38_conic_property(
        &self,
        conic: ConicId,
        property: M38ConicProperty,
    ) -> Result<f64, SketchError> {
        let value = self.conic_value(conic)?;
        let distance = |first, second| -> Result<f64, SketchError> {
            Ok((self.point_position(second)? - self.point_position(first)?).norm())
        };
        match (value.kind(), property) {
            (
                crate::ConicKind::Ellipse {
                    center,
                    major_axis_point,
                    ..
                }
                | crate::ConicKind::EllipticalArc {
                    center,
                    major_axis_point,
                    ..
                },
                M38ConicProperty::MajorAxisLength,
            ) => Ok(2.0 * distance(center, major_axis_point)?),
            (
                crate::ConicKind::Ellipse {
                    center,
                    major_axis_point,
                    minor_axis_ratio,
                }
                | crate::ConicKind::EllipticalArc {
                    center,
                    major_axis_point,
                    minor_axis_ratio,
                    ..
                },
                M38ConicProperty::MinorAxisLength,
            ) => Ok(2.0 * distance(center, major_axis_point)? * minor_axis_ratio),
            (
                crate::ConicKind::Ellipse {
                    center,
                    major_axis_point,
                    minor_axis_ratio,
                }
                | crate::ConicKind::EllipticalArc {
                    center,
                    major_axis_point,
                    minor_axis_ratio,
                    ..
                },
                M38ConicProperty::LinearEccentricity,
            ) => Ok(distance(center, major_axis_point)?
                * (1.0 - minor_axis_ratio * minor_axis_ratio).sqrt()),
            (
                crate::ConicKind::ParabolaSegment { vertex, focus, .. },
                M38ConicProperty::FocalDistance,
            ) => distance(vertex, focus),
            (
                crate::ConicKind::HyperbolaSegment {
                    center,
                    transverse_axis_point,
                    semi_conjugate,
                    ..
                },
                M38ConicProperty::FocalDistance,
            ) => Ok(distance(center, transverse_axis_point)?.hypot(semi_conjugate)),
            (
                crate::ConicKind::HyperbolaSegment {
                    center,
                    transverse_axis_point,
                    ..
                },
                M38ConicProperty::TransverseAxisLength,
            ) => Ok(2.0 * distance(center, transverse_axis_point)?),
            (
                crate::ConicKind::HyperbolaSegment { semi_conjugate, .. },
                M38ConicProperty::ConjugateAxisLength,
            ) => Ok(2.0 * semi_conjugate),
            _ => Err(SketchError::InvalidCurveContact(
                "conic property is not supported for this family",
            )),
        }
    }

    fn path_length_value(
        &self,
        curve: SketchCurve,
        start: f64,
        end: f64,
    ) -> Result<f64, SketchError> {
        const PANELS: u32 = 64;
        let h = (end - start) / f64::from(PANELS);
        let mut sum = 0.0;
        for index in 0..=PANELS {
            let parameter = start + f64::from(index) * h;
            let speed = self
                .evaluate_curve_contact(SketchCurveContact {
                    curve,
                    parameter,
                    neighborhood: CurveContactNeighborhood::Interior,
                })?
                .first_derivative
                .norm();
            if !speed.is_finite() || speed == 0.0 {
                return Err(SketchError::InvalidCurveContact(
                    "path-length curve is not regular",
                ));
            }
            sum += if index == 0 || index == PANELS {
                speed
            } else if index % 2 == 0 {
                2.0 * speed
            } else {
                4.0 * speed
            };
        }
        Ok(h * sum / 3.0)
    }

    fn validate_point_pair(&self, first: PointId, second: PointId) -> Result<(), SketchError> {
        self.point_position(first)?;
        self.point_position(second)?;
        if first == second {
            Err(SketchError::RepeatedPoint)
        } else {
            Ok(())
        }
    }

    pub(crate) fn insert_constraint(&mut self, kind: SketchConstraintKind) -> SketchConstraintId {
        let constraint = self.constraints.insert(SketchConstraint {
            kind,
            ordinal: self.constraints.next_ordinal(),
        });
        self.source_order
            .push(PersistentSource::Constraint(constraint));
        constraint
    }

    pub(crate) fn insert_dimension(
        &mut self,
        kind: DimensionKind,
        mode: DimensionMode,
    ) -> SketchDimensionId {
        let dimension = self.dimensions.insert(SketchDimension {
            kind,
            mode,
            ordinal: self.dimensions.next_ordinal(),
        });
        self.source_order
            .push(PersistentSource::Dimension(dimension));
        dimension
    }
}

#[allow(clippy::too_many_lines)]
fn constraint_references_point(
    kind: SketchConstraintKind,
    point: PointId,
    sketch: &Sketch,
) -> bool {
    match kind {
        SketchConstraintKind::FixedPoint { point: id, .. }
        | SketchConstraintKind::ExternalPoint { point: id, .. }
        | SketchConstraintKind::FixedCoordinate { point: id, .. } => id == point,
        SketchConstraintKind::Coincident { first, second }
        | SketchConstraintKind::HorizontalPoints { first, second }
        | SketchConstraintKind::VerticalPoints { first, second } => {
            first == point || second == point
        }
        SketchConstraintKind::Horizontal { segment }
        | SketchConstraintKind::Vertical { segment } => sketch
            .segment_endpoints(segment)
            .is_ok_and(|(first, second)| first == point || second == point),
        SketchConstraintKind::PointOnLine {
            point: constrained,
            segment,
            ..
        }
        | SketchConstraintKind::Midpoint {
            point: constrained,
            segment,
        } => {
            constrained == point
                || sketch
                    .segment_endpoints(segment)
                    .is_ok_and(|(first, second)| first == point || second == point)
        }
        SketchConstraintKind::PointOnCircle {
            point: constrained,
            circle,
            ..
        } => {
            constrained == point
                || sketch
                    .circles
                    .get(circle)
                    .is_some_and(|value| value.center() == point)
        }
        SketchConstraintKind::PointOnArc {
            point: constrained,
            arc,
            ..
        } => {
            constrained == point
                || sketch
                    .arcs
                    .get(arc)
                    .is_some_and(|value| value.center() == point)
        }
        SketchConstraintKind::PointOnBezier {
            point: constrained,
            bezier,
            ..
        } => {
            constrained == point
                || sketch
                    .beziers
                    .get(bezier)
                    .is_some_and(|curve| curve.controls().contains(&point))
        }
        SketchConstraintKind::PointOnCurve {
            point: constrained,
            contact,
        } => constrained == point || contact.curve.references_point(sketch, point),
        SketchConstraintKind::Parallel { first, second }
        | SketchConstraintKind::Perpendicular { first, second }
        | SketchConstraintKind::EqualSegmentLength { first, second }
        | SketchConstraintKind::Collinear { first, second } => [first, second]
            .into_iter()
            .filter_map(|segment| sketch.segment_endpoints(segment).ok())
            .any(|(start, end)| start == point || end == point),
        SketchConstraintKind::ExternalLineCollinear { segment, .. } => sketch
            .segment_endpoints(segment)
            .is_ok_and(|(start, end)| start == point || end == point),
        SketchConstraintKind::EqualCircleRadius { first, second }
        | SketchConstraintKind::CircleCircleTangency { first, second, .. } => [first, second]
            .into_iter()
            .filter_map(|circle| sketch.circles.get(circle))
            .any(|circle| circle.center() == point),
        SketchConstraintKind::EqualCircleArcRadius { circle, arc } => {
            sketch
                .circles
                .get(circle)
                .is_some_and(|value| value.center() == point)
                || sketch
                    .arcs
                    .get(arc)
                    .is_some_and(|value| value.center() == point)
        }
        SketchConstraintKind::EqualArcRadius { first, second } => [first, second]
            .into_iter()
            .filter_map(|arc| sketch.arcs.get(arc))
            .any(|arc| arc.center() == point),
        SketchConstraintKind::FixedArcAngle { arc, .. } => sketch
            .arcs
            .get(arc)
            .is_some_and(|arc| arc.center() == point),
        SketchConstraintKind::FixedScalar { property, .. } => {
            scalar_ref_references_point(property, point, sketch)
        }
        SketchConstraintKind::EqualScalar { first, second, .. } => {
            scalar_ref_references_point(first, point, sketch)
                || scalar_ref_references_point(second, point, sketch)
        }
        SketchConstraintKind::EqualDistance { first, second } => {
            first.contains(&point) || second.contains(&point)
        }
        SketchConstraintKind::EqualAngle { first, second, .. } => first
            .into_iter()
            .chain(second)
            .filter_map(|segment| sketch.segment_endpoints(segment).ok())
            .any(|(start, end)| start == point || end == point),
        SketchConstraintKind::SymmetricAboutLine {
            first,
            second,
            line,
        } => {
            first == point
                || second == point
                || sketch
                    .segment_endpoints(line)
                    .is_ok_and(|(start, end)| start == point || end == point)
        }
        SketchConstraintKind::PointSymmetry {
            first,
            second,
            center,
        } => first == point || second == point || center == point,
        SketchConstraintKind::LineCircleTangency { line, circle, .. } => {
            sketch
                .segment_endpoints(line)
                .is_ok_and(|(start, end)| start == point || end == point)
                || sketch
                    .circles
                    .get(circle)
                    .is_some_and(|value| value.center() == point)
        }
        SketchConstraintKind::CircleArcTangency { circle, arc, .. } => {
            sketch
                .circles
                .get(circle)
                .is_some_and(|value| value.center() == point)
                || sketch
                    .arcs
                    .get(arc)
                    .is_some_and(|value| value.center() == point)
        }
        SketchConstraintKind::LineBezierTangency { line, bezier, .. } => {
            sketch
                .segment_endpoints(line)
                .is_ok_and(|(start, end)| start == point || end == point)
                || sketch
                    .beziers
                    .get(bezier)
                    .is_some_and(|curve| curve.controls().contains(&point))
        }
        SketchConstraintKind::LineCurveTangency { line, contact, .. } => {
            sketch
                .segment_endpoints(line)
                .is_ok_and(|(start, end)| start == point || end == point)
                || contact.curve.references_point(sketch, point)
        }
        SketchConstraintKind::CurveDirection { line, contact, .. } => {
            sketch
                .segment_endpoints(line)
                .is_ok_and(|(start, end)| start == point || end == point)
                || contact.curve.references_point(sketch, point)
        }
        SketchConstraintKind::CurveCurveContact { first, second }
        | SketchConstraintKind::CurveCurveTangency { first, second, .. }
        | SketchConstraintKind::EqualCurvature { first, second, .. }
        | SketchConstraintKind::EndpointContinuity { first, second, .. } => {
            first.curve.references_point(sketch, point)
                || second.curve.references_point(sketch, point)
        }
        SketchConstraintKind::CurveCurveFillet {
            arc, first, second, ..
        } => {
            first.curve.references_point(sketch, point)
                || second.curve.references_point(sketch, point)
                || sketch
                    .arcs
                    .get(arc)
                    .is_some_and(|value| value.center() == point)
        }
    }
}

fn scalar_ref_references_point(property: SketchScalarRef, point: PointId, sketch: &Sketch) -> bool {
    match property {
        SketchScalarRef::CircleRadius(circle) => sketch
            .circles
            .get(circle)
            .is_some_and(|curve| curve.center() == point),
        SketchScalarRef::ArcRadius(arc) | SketchScalarRef::ArcAngle { arc, .. } => sketch
            .arcs
            .get(arc)
            .is_some_and(|curve| curve.center() == point),
        SketchScalarRef::ConicScalar { conic, .. } => sketch
            .conics
            .get(conic)
            .is_some_and(|curve| curve.points().contains(&point)),
        SketchScalarRef::NurbsWeight { nurbs, .. } => sketch
            .nurbs
            .get(nurbs)
            .is_some_and(|curve| curve.controls().contains(&point)),
    }
}

fn dimension_references_point(kind: DimensionKind, point: PointId, sketch: &Sketch) -> bool {
    match kind {
        DimensionKind::CoordinateDifference { first, second, .. }
        | DimensionKind::PointDistance { first, second, .. } => first == point || second == point,
        DimensionKind::SegmentLength { segment, .. } => sketch
            .segment_endpoints(segment)
            .is_ok_and(|(first, second)| first == point || second == point),
        DimensionKind::CircleRadius { circle, .. }
        | DimensionKind::CircleDiameter { circle, .. } => sketch
            .circles
            .get(circle)
            .is_some_and(|circle| circle.center() == point),
        DimensionKind::ArcRadius { arc, .. } | DimensionKind::ArcDiameter { arc, .. } => sketch
            .arcs
            .get(arc)
            .is_some_and(|arc| arc.center() == point),
        DimensionKind::CircularSweep { arc, .. } | DimensionKind::CircularArcLength { arc, .. } => {
            sketch
                .arcs
                .get(arc)
                .is_some_and(|arc| arc.center() == point)
        }
        DimensionKind::ConicProperty { conic, .. } => sketch
            .conics
            .get(conic)
            .is_some_and(|curve| curve.points().contains(&point)),
        DimensionKind::PathLength { curve, .. } => curve.references_point(sketch, point),
        DimensionKind::EqualPathLength { first, second, .. } => {
            first.references_point(sketch, point) || second.references_point(sketch, point)
        }
        DimensionKind::OrientedAngle { first, second, .. } => [first, second]
            .into_iter()
            .filter_map(|segment| sketch.segment_endpoints(segment).ok())
            .any(|(start, end)| start == point || end == point),
        DimensionKind::SupportingLineOffset {
            source,
            target_segment,
            ..
        }
        | DimensionKind::ExactTranslatedSegmentOffset {
            source,
            target_segment,
            ..
        } => [source, target_segment]
            .into_iter()
            .filter_map(|segment| sketch.segment_endpoints(segment).ok())
            .any(|(start, end)| start == point || end == point),
    }
}

fn constraint_references_segment(kind: SketchConstraintKind, segment: SegmentId) -> bool {
    match kind {
        SketchConstraintKind::Horizontal { segment: id }
        | SketchConstraintKind::Vertical { segment: id }
        | SketchConstraintKind::PointOnLine { segment: id, .. }
        | SketchConstraintKind::Midpoint { segment: id, .. } => id == segment,
        SketchConstraintKind::Parallel { first, second }
        | SketchConstraintKind::Perpendicular { first, second }
        | SketchConstraintKind::EqualSegmentLength { first, second } => {
            first == segment || second == segment
        }
        SketchConstraintKind::SymmetricAboutLine { line, .. }
        | SketchConstraintKind::LineCircleTangency { line, .. }
        | SketchConstraintKind::LineBezierTangency { line, .. } => line == segment,
        SketchConstraintKind::PointOnCurve { contact, .. } => {
            contact.curve.references_segment(segment)
        }
        SketchConstraintKind::LineCurveTangency { line, contact, .. }
        | SketchConstraintKind::CurveDirection { line, contact, .. } => {
            line == segment || contact.curve.references_segment(segment)
        }
        SketchConstraintKind::CurveCurveContact { first, second }
        | SketchConstraintKind::CurveCurveTangency { first, second, .. }
        | SketchConstraintKind::EqualCurvature { first, second, .. }
        | SketchConstraintKind::EndpointContinuity { first, second, .. }
        | SketchConstraintKind::CurveCurveFillet { first, second, .. } => {
            first.curve.references_segment(segment) || second.curve.references_segment(segment)
        }
        _ => false,
    }
}

impl SketchCurve {
    fn references_point(self, sketch: &Sketch, point: PointId) -> bool {
        match self {
            Self::Line { segment, .. } => sketch
                .segment_endpoints(segment)
                .is_ok_and(|(start, end)| start == point || end == point),
            Self::Circle(circle) => sketch
                .circles
                .get(circle)
                .is_some_and(|value| value.center() == point),
            Self::Arc(arc) => sketch
                .arcs
                .get(arc)
                .is_some_and(|value| value.center() == point),
            Self::Bezier(bezier) => sketch
                .beziers
                .get(bezier)
                .is_some_and(|value| value.controls().contains(&point)),
            Self::Conic(conic) => sketch
                .conics
                .get(conic)
                .is_some_and(|value| value.kind().references_point(point)),
            Self::BSpline { spline, .. } => sketch
                .bsplines
                .get(spline)
                .is_some_and(|value| value.controls().contains(&point)),
            Self::Nurbs { nurbs, .. } => sketch
                .nurbs
                .get(nurbs)
                .is_some_and(|value| value.controls().contains(&point)),
        }
    }

    fn references_segment(self, segment: SegmentId) -> bool {
        matches!(self, Self::Line { segment: id, .. } if id == segment)
    }
}

fn dimension_references_segment(kind: DimensionKind, segment: SegmentId) -> bool {
    match kind {
        DimensionKind::SegmentLength { segment: id, .. } => id == segment,
        DimensionKind::OrientedAngle { first, second, .. } => first == segment || second == segment,
        DimensionKind::SupportingLineOffset {
            source,
            target_segment,
            ..
        }
        | DimensionKind::ExactTranslatedSegmentOffset {
            source,
            target_segment,
            ..
        } => source == segment || target_segment == segment,
        _ => false,
    }
}

pub(crate) fn validate_model_scale(model_scale: f64) -> Result<(), SketchError> {
    if model_scale.is_finite() && model_scale > 0.0 {
        Ok(())
    } else {
        Err(SketchError::InvalidModelScale(model_scale))
    }
}

pub(crate) fn validate_point(point: Point2<f64>, context: &'static str) -> Result<(), SketchError> {
    if point.x.is_finite() && point.y.is_finite() {
        Ok(())
    } else {
        Err(SketchError::NonFinitePoint {
            context,
            x: point.x,
            y: point.y,
        })
    }
}

pub(crate) fn validate_finite(value: f64, context: &'static str) -> Result<(), SketchError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(SketchError::NonFiniteValue { context, value })
    }
}

pub(crate) fn validate_dimension_value(value: f64) -> Result<(), SketchError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(SketchError::InvalidDimensionValue(value))
    }
}

fn validate_path_interval(start: f64, end: f64) -> Result<(), SketchError> {
    if start.is_finite() && end.is_finite() && 0.0 <= start && start < end && end <= 1.0 {
        Ok(())
    } else {
        Err(SketchError::InvalidCurveContact(
            "path interval must satisfy 0 <= start < end <= 1",
        ))
    }
}

pub(crate) fn nonempty_label(
    label: impl Into<String>,
    kind: &'static str,
) -> Result<String, SketchError> {
    let label = label.into();
    if label.trim().is_empty() {
        Err(SketchError::EmptyLabel(kind))
    } else {
        Ok(label)
    }
}
