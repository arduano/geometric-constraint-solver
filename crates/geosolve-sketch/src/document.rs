use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// Current on-disk sketch-document schema.
pub const SKETCH_DOCUMENT_VERSION: u32 = 1;
/// Defensive import limit for all persistent objects combined.
pub const MAX_DOCUMENT_OBJECTS: usize = 100_000;
/// Defensive import limit for one polyline.
pub const MAX_POLYLINE_POINTS: usize = 10_000;
/// Defensive control-count limit for one B-spline.
pub const MAX_BSPLINE_CONTROLS: usize = 10_000;
/// Defensive import limit for labels.
pub const MAX_LABEL_BYTES: usize = 1_024;
/// Defensive byte limit applied before JSON deserialization.
pub const MAX_DOCUMENT_JSON_BYTES: usize = 16 * 1024 * 1024;

static DOCUMENT_NONCE: AtomicU32 = AtomicU32::new(1);

/// Opaque persistent identity encoded as exactly 32 lowercase hexadecimal digits.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PersistentId(u128);

impl PersistentId {
    #[must_use]
    pub const fn from_u128(value: u128) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_u128(self) -> u128 {
        self.0
    }
}

impl fmt::Display for PersistentId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:032x}", self.0)
    }
}

impl FromStr for PersistentId {
    type Err = DocumentError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != 32
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(DocumentError::InvalidId(value.to_owned()));
        }
        u128::from_str_radix(value, 16)
            .map(Self)
            .map_err(|_| DocumentError::InvalidId(value.to_owned()))
    }
}

impl Serialize for PersistentId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for PersistentId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

macro_rules! typed_id {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(
            Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub PersistentId);

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl From<$name> for PersistentId {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

typed_id!(DocumentId, "Persistent sketch-document identity.");
typed_id!(DesignPointId, "Persistent design-point identity.");
typed_id!(DesignScalarId, "Persistent design-scalar identity.");
typed_id!(CurveId, "Persistent curve identity.");
typed_id!(ContactId, "Persistent contact-slot identity.");
typed_id!(
    DocumentConstraintId,
    "Persistent geometric-constraint identity."
);
typed_id!(DocumentDimensionId, "Persistent dimension identity.");
typed_id!(
    DocumentSourceId,
    "Persistent source-order and audit identity."
);

/// Validation, persistence, lowering, or guarded-edit error.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DocumentError {
    #[error("invalid persistent ID encoding `{0}`")]
    InvalidId(String),
    #[error("unsupported sketch-document version {actual}; expected {expected}")]
    UnsupportedVersion { actual: u32, expected: u32 },
    #[error("duplicate persistent ID {0}")]
    DuplicateId(PersistentId),
    #[error("unknown {kind} ID {id}")]
    UnknownId {
        kind: &'static str,
        id: PersistentId,
    },
    #[error("invalid document field `{field}`: {message}")]
    InvalidField {
        field: &'static str,
        message: String,
    },
    #[error("contact {contact} parameter {value} is outside [{lower}, {upper}]")]
    ContactParameterOutOfDomain {
        contact: ContactId,
        value: f64,
        lower: f64,
        upper: f64,
    },
    #[error("contact {contact} tangent regularity failure: {source}")]
    ContactRegularity {
        contact: ContactId,
        #[source]
        source: geosolve_geometry::CurveRegularityError,
    },
    #[error("contact {contact} conic evaluation failure: {source}")]
    ContactConicEvaluation {
        contact: ContactId,
        #[source]
        source: geosolve_geometry::ConicEvaluationError,
    },
    #[error("contact {contact} differential evaluation failure: {source}")]
    ContactDifferential {
        contact: ContactId,
        #[source]
        source: geosolve_geometry::CurveDifferentialError,
    },
    #[error("curve {curve} has an invalid conic definition: {source}")]
    ConicDefinition {
        curve: CurveId,
        #[source]
        source: geosolve_geometry::ConicDefinitionError,
    },
    #[error("curve {curve} has an invalid conic evaluation: {source}")]
    ConicEvaluation {
        curve: CurveId,
        #[source]
        source: geosolve_geometry::ConicEvaluationError,
    },
    #[error("curve {curve} has an invalid B-spline definition: {source}")]
    BSplineDefinition {
        curve: CurveId,
        #[source]
        source: geosolve_geometry::BSplineDefinitionError,
    },
    #[error("curve {curve} has an invalid B-spline evaluation: {source}")]
    BSplineEvaluation {
        curve: CurveId,
        #[source]
        source: geosolve_geometry::BSplineEvaluationError,
    },
    #[error("curve {curve} rejected B-spline knot insertion: {source}")]
    BSplineInsertion {
        curve: CurveId,
        #[source]
        source: geosolve_geometry::BSplineInsertionError,
    },
    #[error("curve {curve} has an invalid NURBS definition: {source}")]
    NurbsDefinition {
        curve: CurveId,
        #[source]
        source: geosolve_geometry::NurbsDefinitionError,
    },
    #[error("curve {curve} has an invalid NURBS evaluation: {source}")]
    NurbsEvaluation {
        curve: CurveId,
        #[source]
        source: geosolve_geometry::NurbsEvaluationError,
    },
    #[error("curve {curve} rejected NURBS knot insertion: {source}")]
    NurbsInsertion {
        curve: CurveId,
        #[source]
        source: geosolve_geometry::NurbsInsertionError,
    },
    #[error("document resource limit exceeded for {resource}: {actual} > {limit}")]
    ResourceLimit {
        resource: &'static str,
        actual: usize,
        limit: usize,
    },
    #[error("object {0} is still referenced")]
    ObjectInUse(PersistentId),
    #[error("persistent ID space is exhausted")]
    IdExhausted,
    #[error("invalid JSON document: {0}")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Sketch(#[from] crate::SketchError),
}

/// Typed accepted-document curve evaluation failure.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DocumentCurveEvaluationError {
    #[error(transparent)]
    Document(#[from] DocumentError),
    #[error(transparent)]
    Curve(#[from] geosolve_geometry::CurveEvaluationError),
    #[error(transparent)]
    ConicDefinition(#[from] geosolve_geometry::ConicDefinitionError),
    #[error(transparent)]
    ConicEvaluation(#[from] geosolve_geometry::ConicEvaluationError),
    #[error(transparent)]
    BSplineDefinition(#[from] geosolve_geometry::BSplineDefinitionError),
    #[error(transparent)]
    BSplineEvaluation(#[from] geosolve_geometry::BSplineEvaluationError),
    #[error(transparent)]
    NurbsDefinition(#[from] geosolve_geometry::NurbsDefinitionError),
    #[error(transparent)]
    NurbsEvaluation(#[from] geosolve_geometry::NurbsEvaluationError),
}

/// Typed differential measurement failure from accepted persistent geometry.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DocumentCurveMeasurementError {
    #[error(transparent)]
    Evaluation(#[from] DocumentCurveEvaluationError),
    #[error(transparent)]
    Differential(#[from] geosolve_geometry::CurveDifferentialError),
}

/// Immutable projection of one draggable curve-trim endpoint onto its owned scalar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DocumentTrimProjection {
    pub scalar: DesignScalarId,
    pub value: f64,
}

/// Persistent identities changed by one accepted B-spline knot insertion.
#[derive(Clone, Debug, PartialEq)]
pub struct DocumentBSplineInsertion {
    pub curve: CurveId,
    pub new_control: DesignPointId,
    pub new_span_id: Option<u32>,
    pub migrated_contacts: Vec<ContactId>,
}

/// Persistent identities changed by one accepted NURBS knot insertion.
#[derive(Clone, Debug, PartialEq)]
pub struct DocumentNurbsInsertion {
    pub curve: CurveId,
    pub new_control: DesignPointId,
    pub new_weight: DesignScalarId,
    pub new_span_id: Option<u32>,
    pub migrated_contacts: Vec<ContactId>,
}

/// Typed failure to project a world target onto a persistent curve-trim scalar.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DocumentTrimProjectionError {
    #[error(transparent)]
    Document(#[from] DocumentError),
    #[error("curve {curve} has invalid conic geometry: {source}")]
    ConicDefinition {
        curve: CurveId,
        #[source]
        source: geosolve_geometry::ConicDefinitionError,
    },
    #[error("curve {curve} does not support trim-endpoint projection")]
    UnsupportedCurve { curve: CurveId },
    #[error("trim-endpoint projection target for curve {curve} must be finite")]
    NonFiniteTarget { curve: CurveId },
    #[error("trim-endpoint projection target for curve {curve} is its ambiguous center")]
    AmbiguousCenterTarget { curve: CurveId },
    #[error("trim-endpoint projection for curve {curve} produced a non-finite value")]
    NonFiniteResult { curve: CurveId },
}

/// Physical meaning carried by a persistent scalar.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScalarUnit {
    Length,
    Angle,
    Parameter,
}

/// Explicit scalar-domain state used by dimensions, radii, and contacts.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScalarDomain {
    Finite,
    Positive,
    Bounded { lower: f64, upper: f64 },
    Periodic { period: f64 },
}

/// One persistent solver scalar.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DesignScalar {
    pub id: DesignScalarId,
    pub label: String,
    pub value: f64,
    pub unit: ScalarUnit,
    pub domain: ScalarDomain,
}

/// One persistent Cartesian design point.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DesignPoint {
    pub id: DesignPointId,
    pub label: String,
    pub position: [f64; 2],
}

/// Explicit circular-arc traversal branch.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentArcSweep {
    CounterClockwise,
    Clockwise,
}

/// Explicit selected branch of a persistent hyperbola segment.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentHyperbolaBranch {
    Positive,
    Negative,
}

/// Serialized topology of a persistent non-rational B-spline.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentBSplineForm {
    Clamped,
    Periodic,
}

/// Explicit adjacent B-spline span transition direction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentBSplineSpanDirection {
    Previous,
    Next,
}

impl From<DocumentBSplineForm> for geosolve_geometry::BSplineForm {
    fn from(value: DocumentBSplineForm) -> Self {
        match value {
            DocumentBSplineForm::Clamped => Self::Clamped,
            DocumentBSplineForm::Periodic => Self::Periodic,
        }
    }
}

/// Closed alpha curve-definition set.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CurveDefinition {
    Line {
        start: DesignPointId,
        end: DesignPointId,
        branch_direction: [f64; 2],
    },
    Polyline {
        points: Vec<DesignPointId>,
        closed: bool,
        branch_directions: Vec<[f64; 2]>,
    },
    Circle {
        center: DesignPointId,
        radius: DesignScalarId,
    },
    CircularArc {
        center: DesignPointId,
        radius: DesignScalarId,
        start_angle: DesignScalarId,
        end_angle: DesignScalarId,
        sweep: DocumentArcSweep,
    },
    QuadraticBezier {
        controls: [DesignPointId; 3],
    },
    CubicBezier {
        controls: [DesignPointId; 4],
    },
    Ellipse {
        center: DesignPointId,
        major_axis_point: DesignPointId,
        minor_axis_ratio: DesignScalarId,
    },
    EllipticalArc {
        center: DesignPointId,
        major_axis_point: DesignPointId,
        minor_axis_ratio: DesignScalarId,
        start_angle: DesignScalarId,
        end_angle: DesignScalarId,
        sweep: DocumentArcSweep,
    },
    RationalQuadraticConic {
        start: DesignPointId,
        weighted_middle: [f64; 2],
        middle_weight: DesignScalarId,
        end: DesignPointId,
    },
    ParabolaSegment {
        vertex: DesignPointId,
        focus: DesignPointId,
        trim_start: DesignScalarId,
        trim_end: DesignScalarId,
    },
    HyperbolaSegment {
        center: DesignPointId,
        transverse_axis_point: DesignPointId,
        semi_conjugate: DesignScalarId,
        branch: DocumentHyperbolaBranch,
        trim_start: DesignScalarId,
        trim_end: DesignScalarId,
    },
    BSpline {
        form: DocumentBSplineForm,
        degree: u32,
        controls: Vec<DesignPointId>,
        knots: Vec<f64>,
        span_ids: Vec<u32>,
        next_span_id: u32,
    },
    Nurbs {
        form: DocumentBSplineForm,
        degree: u32,
        controls: Vec<DesignPointId>,
        weights: Vec<DesignScalarId>,
        gauge_weight: DesignScalarId,
        knots: Vec<f64>,
        span_ids: Vec<u32>,
        next_span_id: u32,
    },
}

/// One persistent curve entity.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DesignCurve {
    pub id: CurveId,
    pub label: String,
    pub definition: CurveDefinition,
}

/// Semantic selection of one directed segment or stable family-local curve span.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CurveSpan {
    pub curve: CurveId,
    pub segment: u32,
}

/// Endpoint selected by a semantic curve feature.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureEndpoint {
    Start,
    End,
}

/// Semantic geometry reference independent of runtime storage layout.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FeatureRef {
    Point {
        point: DesignPointId,
    },
    CurveEndpoint {
        curve: CurveId,
        endpoint: FeatureEndpoint,
    },
    CurveCenter {
        curve: CurveId,
    },
    CurveAxis {
        curve: CurveId,
    },
    CurveControl {
        curve: CurveId,
        index: u32,
    },
    CurveFocus {
        curve: CurveId,
        index: u32,
    },
    FixedCurveLocation {
        contact: ContactId,
    },
}

/// One finite point-valued feature exposed by the persistent conic query seam.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentConicFeature {
    Center,
    Focus { index: u32 },
    MajorAxisEndpoint { endpoint: FeatureEndpoint },
    MinorAxisEndpoint { endpoint: FeatureEndpoint },
    BoundedEndpoint { endpoint: FeatureEndpoint },
    SelectedBranchVertex,
}

/// One finite scalar measurement exposed by the persistent conic query seam.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentConicMeasurement {
    MajorAxisLength,
    MinorAxisLength,
    LinearEccentricity,
    FocalDistance,
    TransverseAxisLength,
    ConjugateAxisLength,
}

/// Typed persistent conic feature/measurement query failure.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DocumentConicQueryError {
    #[error(transparent)]
    Document(#[from] DocumentError),
    #[error(transparent)]
    Definition(#[from] geosolve_geometry::ConicDefinitionError),
    #[error(transparent)]
    Evaluation(#[from] geosolve_geometry::ConicEvaluationError),
    #[error("feature {feature:?} is unsupported for curve {curve}")]
    UnsupportedFeature {
        curve: CurveId,
        feature: DocumentConicFeature,
    },
    #[error("measurement {measurement:?} is unsupported for curve {curve}")]
    UnsupportedMeasurement {
        curve: CurveId,
        measurement: DocumentConicMeasurement,
    },
    #[error("conic query for curve {curve} returned a non-finite value")]
    NonFiniteResult { curve: CurveId },
}

impl CurveSpan {
    #[must_use]
    pub const fn line(curve: CurveId) -> Self {
        Self { curve, segment: 0 }
    }
}

/// Contact parameter topology.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ContactDomain {
    SupportingLine,
    Bounded { lower: f64, upper: f64 },
    Periodic { period: f64 },
}

/// Explicit selected neighborhood on a bounded curve.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContactNeighborhood {
    Interior,
    Local { lower: f64, upper: f64 },
    Start,
    End,
}

/// Explicit tangent orientation at a selected contact.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TangentOrientation {
    Aligned,
    Opposed,
}

/// Directed normal side relative to increasing curve parameter.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentCurveNormalSide {
    Left,
    Right,
}

/// Explicit line-to-curve differential direction relation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DocumentCurveDirectionRelation {
    Tangent { orientation: TangentOrientation },
    Normal { side: DocumentCurveNormalSide },
}

/// Explicit smooth signed equation used for equal-curvature behavior.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentCurveCurvatureRelation {
    Signed,
    MagnitudeSameSign,
    MagnitudeOppositeSign,
}

/// Ordered incoming/outgoing endpoint continuity policy.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DocumentCurveContinuity {
    G0,
    G1,
    G2,
    ParametricC2 { first_rate: f64, second_rate: f64 },
}

/// Equation-free differential measurement available at a persistent contact.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentCurveMeasurementKind {
    SignedCurvature,
    UnsignedCurvature,
    OsculatingRadius,
}

/// Persistent semantic contact with independent parameter identity.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContactSlot {
    pub id: ContactId,
    pub label: String,
    pub curve: CurveSpan,
    pub parameter: DesignScalarId,
    pub domain: ContactDomain,
    pub winding: i32,
    pub neighborhood: ContactNeighborhood,
    pub tangent_orientation: Option<TangentOrientation>,
}

/// Validated fields used to create one persistent contact slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactDefinition {
    pub curve: CurveSpan,
    pub parameter: DesignScalarId,
    pub domain: ContactDomain,
    pub winding: i32,
    pub neighborhood: ContactNeighborhood,
    pub tangent_orientation: Option<TangentOrientation>,
}

/// One atomic accepted-state update for a persistent contact slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactStateEdit {
    pub contact: ContactId,
    pub value: f64,
    pub winding: i32,
    pub neighborhood: ContactNeighborhood,
    pub tangent_orientation: Option<TangentOrientation>,
}

/// Cartesian coordinate selected by a persistent fixed-coordinate source.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentCoordinateAxis {
    X,
    Y,
}

/// Selected side of a directed line.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentLineSide {
    Left,
    Right,
}

/// Internal circle-tangency containment branch.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentCircleContainment {
    FirstContainsSecond,
    SecondContainsFirst,
}

/// Explicit circle-circle tangency branch.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DocumentCircleTangencyMode {
    External,
    Internal {
        containment: DocumentCircleContainment,
    },
}

/// Explicit radial side of circle-to-arc tangency.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentArcTangencySide {
    OutsideArc,
    InsideArc,
}

/// Closed alpha geometric-constraint set expressed only in semantic IDs.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DocumentConstraintDefinition {
    FixedPoint {
        point: DesignPointId,
        target: [f64; 2],
    },
    FixedCoordinate {
        point: DesignPointId,
        axis: DocumentCoordinateAxis,
        target: f64,
    },
    Coincident {
        first: DesignPointId,
        second: DesignPointId,
    },
    Horizontal {
        line: CurveSpan,
    },
    Vertical {
        line: CurveSpan,
    },
    PointOnCurve {
        point: DesignPointId,
        contact: ContactId,
    },
    Parallel {
        first: CurveSpan,
        second: CurveSpan,
    },
    Perpendicular {
        first: CurveSpan,
        second: CurveSpan,
    },
    EqualLength {
        first: CurveSpan,
        second: CurveSpan,
    },
    EqualRadius {
        first: CurveId,
        second: CurveId,
    },
    Midpoint {
        point: DesignPointId,
        line: CurveSpan,
    },
    SymmetricAboutLine {
        first: DesignPointId,
        second: DesignPointId,
        line: CurveSpan,
    },
    LineCircleTangency {
        line_contact: ContactId,
        circle_contact: ContactId,
        side: DocumentLineSide,
    },
    CircleCircleTangency {
        first: CurveId,
        second: CurveId,
        mode: DocumentCircleTangencyMode,
        center_direction: [f64; 2],
    },
    CircleArcTangency {
        circle_contact: ContactId,
        arc_contact: ContactId,
        side: DocumentArcTangencySide,
    },
    LineCurveTangency {
        line: CurveSpan,
        endpoint: FeatureEndpoint,
        curve_contact: ContactId,
    },
    CurveCurveContact {
        first_contact: ContactId,
        second_contact: ContactId,
    },
    CurveCurveTangency {
        first_contact: ContactId,
        second_contact: ContactId,
    },
    CurveDirection {
        line: CurveSpan,
        curve_contact: ContactId,
        relation: DocumentCurveDirectionRelation,
    },
    EqualCurvature {
        first_contact: ContactId,
        second_contact: ContactId,
        relation: DocumentCurveCurvatureRelation,
    },
    EndpointContinuity {
        first_contact: ContactId,
        second_contact: ContactId,
        continuity: DocumentCurveContinuity,
    },
}

/// One persistent geometric source and its independent audit identity.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentConstraint {
    pub id: DocumentConstraintId,
    pub source_id: DocumentSourceId,
    pub label: String,
    pub suppressed: bool,
    pub definition: DocumentConstraintDefinition,
}

/// Whether a persistent dimension contributes a hard equation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentDimensionMode {
    Driving,
    Reference,
}

/// Orientation used to measure one persistent angle dimension.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentAngleOrientation {
    CounterClockwise,
    Clockwise,
}

/// Closed M11 dimension-definition set.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DocumentDimensionDefinition {
    PointDistance {
        first: DesignPointId,
        second: DesignPointId,
        target: DesignScalarId,
    },
    CurveLength {
        curve: CurveSpan,
        target: DesignScalarId,
    },
    Radius {
        curve: CurveId,
        target: DesignScalarId,
    },
    Diameter {
        curve: CurveId,
        target: DesignScalarId,
    },
    OrientedAngle {
        first: CurveSpan,
        second: CurveSpan,
        target: DesignScalarId,
        orientation: DocumentAngleOrientation,
    },
}

/// One persistent dimension source.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentDimension {
    pub id: DocumentDimensionId,
    pub source_id: DocumentSourceId,
    pub label: String,
    pub mode: DocumentDimensionMode,
    pub suppressed: bool,
    pub definition: DocumentDimensionDefinition,
}

/// Any deletable persistent object identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DocumentObjectId {
    Point(DesignPointId),
    Scalar(DesignScalarId),
    Curve(CurveId),
    Contact(ContactId),
    Constraint(DocumentConstraintId),
    Dimension(DocumentDimensionId),
}

/// IDs created by the rectangle command macro.
#[derive(Clone, Debug, PartialEq)]
pub struct RectangleIds {
    pub points: [DesignPointId; 4],
    pub curves: [CurveId; 4],
    pub anchor: DocumentConstraintId,
    pub constraints: [DocumentConstraintId; 5],
    pub dimensions: [DocumentDimensionId; 2],
    pub targets: [DesignScalarId; 2],
}

/// Versioned persistent sketch graph. Runtime solver IDs never appear here.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SketchDocument {
    version: u32,
    id: DocumentId,
    next_id: PersistentId,
    model_scale: f64,
    points: Vec<DesignPoint>,
    scalars: Vec<DesignScalar>,
    curves: Vec<DesignCurve>,
    contacts: Vec<ContactSlot>,
    constraints: Vec<DocumentConstraint>,
    dimensions: Vec<DocumentDimension>,
    source_order: Vec<DocumentSourceId>,
}

#[derive(Deserialize)]
struct DocumentHeader {
    version: u32,
}

impl SketchDocument {
    /// Creates an empty version-one document.
    ///
    /// # Errors
    ///
    /// Returns an error for a nonpositive or non-finite model scale.
    pub fn new(model_scale: f64) -> Result<Self, DocumentError> {
        Self::with_id(model_scale, fresh_document_id())
    }

    /// Creates an empty document with a caller-supplied persistent namespace identity.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero/exhausted ID or invalid model scale.
    pub fn with_id(model_scale: f64, id: DocumentId) -> Result<Self, DocumentError> {
        finite_positive(model_scale, "model_scale")?;
        if id.0.as_u128() == 0 {
            return invalid("document id", "must be nonzero");
        }
        let next_id = PersistentId(
            id.0.as_u128()
                .checked_add(1)
                .ok_or(DocumentError::IdExhausted)?,
        );
        Ok(Self {
            version: SKETCH_DOCUMENT_VERSION,
            id,
            next_id,
            model_scale,
            points: Vec::new(),
            scalars: Vec::new(),
            curves: Vec::new(),
            contacts: Vec::new(),
            constraints: Vec::new(),
            dimensions: Vec::new(),
            source_order: Vec::new(),
        })
    }

    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    #[must_use]
    pub const fn id(&self) -> DocumentId {
        self.id
    }

    #[must_use]
    pub const fn model_scale(&self) -> f64 {
        self.model_scale
    }

    #[must_use]
    pub fn points(&self) -> &[DesignPoint] {
        &self.points
    }

    #[must_use]
    pub fn scalars(&self) -> &[DesignScalar] {
        &self.scalars
    }

    #[must_use]
    pub fn curves(&self) -> &[DesignCurve] {
        &self.curves
    }

    #[must_use]
    pub fn contacts(&self) -> &[ContactSlot] {
        &self.contacts
    }

    #[must_use]
    pub fn constraints(&self) -> &[DocumentConstraint] {
        &self.constraints
    }

    #[must_use]
    pub fn dimensions(&self) -> &[DocumentDimension] {
        &self.dimensions
    }

    #[must_use]
    pub fn source_order(&self) -> &[DocumentSourceId] {
        &self.source_order
    }

    #[must_use]
    pub fn point(&self, id: DesignPointId) -> Option<&DesignPoint> {
        self.points.iter().find(|value| value.id == id)
    }

    #[must_use]
    pub fn scalar(&self, id: DesignScalarId) -> Option<&DesignScalar> {
        self.scalars.iter().find(|value| value.id == id)
    }

    #[must_use]
    pub fn curve(&self, id: CurveId) -> Option<&DesignCurve> {
        self.curves.iter().find(|value| value.id == id)
    }

    #[must_use]
    pub fn contact(&self, id: ContactId) -> Option<&ContactSlot> {
        self.contacts.iter().find(|value| value.id == id)
    }

    /// Evaluates one persisted contact against the document's accepted geometry.
    ///
    /// # Errors
    ///
    /// Returns a document reference error or the geometry crate's typed domain/regularity error.
    pub fn evaluate_contact_jet(
        &self,
        id: ContactId,
    ) -> Result<geosolve_geometry::CurveJet2, DocumentCurveEvaluationError> {
        let contact = self.require_contact(id)?;
        let parameter = contact_total_value(contact, self.require_scalar(contact.parameter)?.value);
        self.evaluate_curve_jet_in_domain(contact.curve, parameter, contact.domain)
    }

    /// Measures differential geometry at one accepted persistent contact.
    ///
    /// This derives differential data from the independently reconstructed immutable
    /// curve jet and adds no document or curve-family equation.
    ///
    /// # Errors
    ///
    /// Returns typed evaluation, zero-speed, unrepresentable-curvature, or undefined
    /// osculating-radius failures.
    pub fn measure_curve_contact(
        &self,
        contact: ContactId,
        kind: DocumentCurveMeasurementKind,
    ) -> Result<f64, DocumentCurveMeasurementError> {
        let differential = self.evaluate_contact_jet(contact)?.differential()?;
        Ok(match kind {
            DocumentCurveMeasurementKind::SignedCurvature => differential.signed_curvature,
            DocumentCurveMeasurementKind::UnsignedCurvature => differential.unsigned_curvature(),
            DocumentCurveMeasurementKind::OsculatingRadius => differential.osculating_radius()?,
        })
    }

    /// Evaluates one accepted curve span at an arbitrary rendering/query parameter.
    ///
    /// Bounded curves use `[0, 1]`; circles and full ellipses use an unwrapped angle.
    ///
    /// # Errors
    ///
    /// Returns a document reference error or the geometry crate's typed domain/regularity error.
    pub fn evaluate_curve_jet(
        &self,
        span: CurveSpan,
        parameter: f64,
    ) -> Result<geosolve_geometry::CurveJet2, DocumentCurveEvaluationError> {
        let curve = self.validate_span(span)?;
        let domain = match &curve.definition {
            CurveDefinition::Circle { .. } | CurveDefinition::Ellipse { .. } => {
                ContactDomain::Periodic {
                    period: std::f64::consts::TAU,
                }
            }
            _ => ContactDomain::Bounded {
                lower: 0.0,
                upper: 1.0,
            },
        };
        self.evaluate_curve_jet_in_domain(span, parameter, domain)
    }

    /// Returns every semantic span in curve order.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing curve or an unrepresentable segment ordinal.
    pub fn curve_spans(&self, curve: CurveId) -> Result<Vec<CurveSpan>, DocumentError> {
        let curve_value = self.curve(curve).ok_or_else(|| unknown("curve", curve.0))?;
        match &curve_value.definition {
            CurveDefinition::BSpline { span_ids, .. } | CurveDefinition::Nurbs { span_ids, .. } => {
                Ok(span_ids
                    .iter()
                    .copied()
                    .map(|segment| CurveSpan { curve, segment })
                    .collect())
            }
            definition => (0..curve_segment_count(definition))
                .map(|index| {
                    Ok(CurveSpan {
                        curve,
                        segment: u32::try_from(index).map_err(|_| {
                            DocumentError::ResourceLimit {
                                resource: "curve span index",
                                actual: index,
                                limit: u32::MAX as usize,
                            }
                        })?,
                    })
                })
                .collect(),
        }
    }

    /// Reports guaranteed continuity at one native B-spline or NURBS knot.
    ///
    /// # Errors
    ///
    /// Rejects a missing/non-spline curve, malformed topology, or invalid parameter.
    pub fn bspline_continuity_at(
        &self,
        curve: CurveId,
        parameter: f64,
    ) -> Result<Option<geosolve_geometry::BSplineContinuity>, DocumentCurveEvaluationError> {
        let definition = &self
            .curve(curve)
            .ok_or_else(|| unknown("curve", curve.0))?
            .definition;
        if !matches!(
            definition,
            CurveDefinition::BSpline { .. } | CurveDefinition::Nurbs { .. }
        ) {
            return Err(DocumentError::InvalidField {
                field: "curve",
                message: "expected a B-spline or NURBS curve".into(),
            }
            .into());
        }
        Ok(Self::spline_basis(definition)?.continuity_at(parameter)?)
    }

    fn spline_basis(
        definition: &CurveDefinition,
    ) -> Result<geosolve_geometry::BSplineBasis, DocumentCurveEvaluationError> {
        let (CurveDefinition::BSpline {
            form,
            degree,
            controls,
            knots,
            ..
        }
        | CurveDefinition::Nurbs {
            form,
            degree,
            controls,
            knots,
            ..
        }) = definition
        else {
            return Err(DocumentError::InvalidField {
                field: "curve",
                message: "expected a B-spline or NURBS curve".into(),
            }
            .into());
        };
        Ok(match form {
            DocumentBSplineForm::Clamped => geosolve_geometry::BSplineBasis::try_clamped(
                *degree,
                controls.len(),
                knots.clone(),
            )?,
            DocumentBSplineForm::Periodic => geosolve_geometry::BSplineBasis::try_periodic(
                *degree,
                controls.len(),
                knots.clone(),
            )?,
        })
    }

    fn bspline_geometry(
        &self,
        definition: &CurveDefinition,
    ) -> Result<geosolve_geometry::BSplineCurve2, DocumentCurveEvaluationError> {
        let CurveDefinition::BSpline { controls, .. } = definition else {
            return Err(DocumentError::InvalidField {
                field: "curve",
                message: "expected a B-spline curve".into(),
            }
            .into());
        };
        let points = controls
            .iter()
            .map(|control| {
                let point = self.require_point(*control)?;
                Ok(geosolve_geometry::Point2::new(
                    point.position[0],
                    point.position[1],
                ))
            })
            .collect::<Result<Vec<_>, DocumentError>>()?;
        Ok(geosolve_geometry::BSplineCurve2::try_new(
            Self::spline_basis(definition)?,
            points,
        )?)
    }

    fn nurbs_geometry(
        &self,
        definition: &CurveDefinition,
    ) -> Result<geosolve_geometry::NurbsCurve2, DocumentCurveEvaluationError> {
        let CurveDefinition::Nurbs {
            controls, weights, ..
        } = definition
        else {
            return Err(DocumentError::InvalidField {
                field: "curve",
                message: "expected a NURBS curve".into(),
            }
            .into());
        };
        let points = controls
            .iter()
            .map(|control| {
                let point = self.require_point(*control)?;
                Ok(geosolve_geometry::Point2::new(
                    point.position[0],
                    point.position[1],
                ))
            })
            .collect::<Result<Vec<_>, DocumentError>>()?;
        let weights = weights
            .iter()
            .map(|weight| Ok(self.require_scalar(*weight)?.value))
            .collect::<Result<Vec<_>, DocumentError>>()?;
        Ok(geosolve_geometry::NurbsCurve2::try_new(
            Self::spline_basis(definition)?,
            points,
            weights,
        )?)
    }

    fn spline_span_index(
        definition: &CurveDefinition,
        semantic_id: u32,
    ) -> Result<geosolve_geometry::BSplineSpanIndex, DocumentCurveEvaluationError> {
        let (CurveDefinition::BSpline { span_ids, .. } | CurveDefinition::Nurbs { span_ids, .. }) =
            definition
        else {
            return Err(DocumentError::InvalidField {
                field: "curve",
                message: "expected a B-spline or NURBS curve".into(),
            }
            .into());
        };
        let ordinal = span_ids
            .iter()
            .position(|candidate| *candidate == semantic_id)
            .ok_or_else(|| DocumentError::InvalidField {
                field: "curve span",
                message: "semantic span ID is outside the B-spline".into(),
            })?;
        Self::spline_basis(definition)?
            .spans()
            .get(ordinal)
            .map(geosolve_geometry::BSplineSpan::index)
            .ok_or_else(|| {
                DocumentError::InvalidField {
                    field: "curve span",
                    message: "semantic span mapping is inconsistent".into(),
                }
                .into()
            })
    }

    /// Projects a world target onto one curve's existing start/end trim scalar.
    ///
    /// Angular results are unwrapped near the selected endpoint's current scalar. The method does
    /// not clamp, reorder, swap, allocate, or change explicit sweep/branch state.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a missing or unsupported curve, non-finite input/result, invalid
    /// conic geometry, or an angular target exactly at the curve center.
    #[allow(clippy::too_many_lines)]
    pub fn project_curve_trim_endpoint(
        &self,
        curve: CurveId,
        endpoint: FeatureEndpoint,
        target: [f64; 2],
    ) -> Result<DocumentTrimProjection, DocumentTrimProjectionError> {
        use crate::ConicGeometry as G;

        let definition = &self
            .curve(curve)
            .ok_or_else(|| unknown("curve", curve.0))?
            .definition;
        if !matches!(
            definition,
            CurveDefinition::CircularArc { .. }
                | CurveDefinition::EllipticalArc { .. }
                | CurveDefinition::ParabolaSegment { .. }
                | CurveDefinition::HyperbolaSegment { .. }
        ) {
            return Err(DocumentTrimProjectionError::UnsupportedCurve { curve });
        }
        if !target.iter().all(|value| value.is_finite()) {
            return Err(DocumentTrimProjectionError::NonFiniteTarget { curve });
        }

        let select_scalar = |start, end| match endpoint {
            FeatureEndpoint::Start => start,
            FeatureEndpoint::End => end,
        };
        let (scalar, value) = match definition {
            CurveDefinition::CircularArc {
                center,
                start_angle,
                end_angle,
                ..
            } => {
                let scalar = select_scalar(*start_angle, *end_angle);
                let seed = self.require_scalar(scalar)?.value;
                let center = self.require_point(*center)?.position;
                let difference = angular_target_difference(curve, center, target)?;
                let principal = difference[1].atan2(difference[0]);
                (scalar, crate::curves::unwrap_near(principal, seed))
            }
            CurveDefinition::EllipticalArc {
                start_angle,
                end_angle,
                ..
            } => {
                let scalar = select_scalar(*start_angle, *end_angle);
                let seed = self.require_scalar(scalar)?.value;
                let geometry = self
                    .conic_geometry(definition)
                    .map_err(|error| document_trim_projection_geometry_error(curve, error))?;
                let G::EllipticalArc(arc) = geometry else {
                    unreachable!("elliptical-arc definition reconstructed another conic family")
                };
                let ellipse = arc.ellipse();
                let difference = angular_target_difference(
                    curve,
                    [ellipse.center().x, ellipse.center().y],
                    target,
                )?;
                let difference = geosolve_geometry::Vector2::new(difference[0], difference[1]);
                let major =
                    difference.dot(&ellipse.directed_major_axis().vector()) / ellipse.semi_major();
                let minor =
                    difference.dot(&ellipse.directed_minor_axis().vector()) / ellipse.semi_minor();
                if !major.is_finite() || !minor.is_finite() {
                    return Err(DocumentTrimProjectionError::NonFiniteResult { curve });
                }
                if major == 0.0 && minor == 0.0 {
                    return Err(DocumentTrimProjectionError::AmbiguousCenterTarget { curve });
                }
                (scalar, crate::curves::unwrap_near(minor.atan2(major), seed))
            }
            CurveDefinition::ParabolaSegment {
                trim_start,
                trim_end,
                ..
            } => {
                let scalar = select_scalar(*trim_start, *trim_end);
                let geometry = self
                    .conic_geometry(definition)
                    .map_err(|error| document_trim_projection_geometry_error(curve, error))?;
                let G::ParabolaSegment(parabola) = geometry else {
                    unreachable!("parabola definition reconstructed another conic family")
                };
                let target = geosolve_geometry::Point2::new(target[0], target[1]);
                let normal = parabola.opening_axis().left_normal().vector();
                let value =
                    (target - parabola.vertex()).dot(&normal) / (2.0 * parabola.focal_length());
                (scalar, value)
            }
            CurveDefinition::HyperbolaSegment {
                trim_start,
                trim_end,
                ..
            } => {
                let scalar = select_scalar(*trim_start, *trim_end);
                let geometry = self
                    .conic_geometry(definition)
                    .map_err(|error| document_trim_projection_geometry_error(curve, error))?;
                let G::HyperbolaSegment(hyperbola) = geometry else {
                    unreachable!("hyperbola definition reconstructed another conic family")
                };
                let target = geosolve_geometry::Point2::new(target[0], target[1]);
                let conjugate = hyperbola.conjugate_axis().vector();
                let value = ((target - hyperbola.center()).dot(&conjugate)
                    / hyperbola.semi_conjugate())
                .asinh();
                (scalar, value)
            }
            _ => unreachable!("unsupported curve families returned above"),
        };
        if !value.is_finite() {
            return Err(DocumentTrimProjectionError::NonFiniteResult { curve });
        }
        Ok(DocumentTrimProjection { scalar, value })
    }

    fn evaluate_curve_jet_in_domain(
        &self,
        span: CurveSpan,
        parameter: f64,
        domain: ContactDomain,
    ) -> Result<geosolve_geometry::CurveJet2, DocumentCurveEvaluationError> {
        let curve = self.validate_span(span)?;
        let point = |id: DesignPointId| -> Result<geosolve_geometry::Point2<f64>, DocumentError> {
            let value = self.require_point(id)?;
            Ok(geosolve_geometry::Point2::new(
                value.position[0],
                value.position[1],
            ))
        };
        Ok(match &curve.definition {
            CurveDefinition::Line { start, end, .. } => geosolve_geometry::line_jet(
                point(*start)?,
                point(*end)?,
                geometry_line_domain(domain)?,
                parameter,
            )?,
            CurveDefinition::Polyline { points, closed, .. } => {
                let index = span.segment as usize;
                let next = if index + 1 == points.len() {
                    if *closed {
                        0
                    } else {
                        return Err(DocumentError::InvalidField {
                            field: "contact.curve",
                            message: "polyline contact segment is invalid".into(),
                        }
                        .into());
                    }
                } else {
                    index + 1
                };
                geosolve_geometry::line_jet(
                    point(points[index])?,
                    point(points[next])?,
                    geometry_line_domain(domain)?,
                    parameter,
                )?
            }
            CurveDefinition::Circle { center, radius } => geosolve_geometry::circle_jet(
                point(*center)?,
                self.require_scalar(*radius)?.value,
                parameter,
            )?,
            CurveDefinition::CircularArc {
                center,
                radius,
                start_angle,
                end_angle,
                sweep,
            } => geosolve_geometry::circular_arc_jet(
                point(*center)?,
                self.require_scalar(*radius)?.value,
                self.require_scalar(*start_angle)?.value,
                document_arc_signed_sweep(
                    self.require_scalar(*start_angle)?.value,
                    self.require_scalar(*end_angle)?.value,
                    *sweep,
                )?,
                parameter,
            )?,
            CurveDefinition::QuadraticBezier { controls } => {
                geosolve_geometry::quadratic_bezier_jet(
                    [
                        point(controls[0])?,
                        point(controls[1])?,
                        point(controls[2])?,
                    ],
                    parameter,
                )?
            }
            CurveDefinition::CubicBezier { controls } => geosolve_geometry::cubic_bezier_jet(
                [
                    point(controls[0])?,
                    point(controls[1])?,
                    point(controls[2])?,
                    point(controls[3])?,
                ],
                parameter,
            )?,
            definition @ CurveDefinition::BSpline { .. } => {
                let span_index = Self::spline_span_index(definition, span.segment)?;
                self.bspline_geometry(definition)?
                    .jet_on_span(span_index, parameter)?
            }
            definition @ CurveDefinition::Nurbs { .. } => {
                let span_index = Self::spline_span_index(definition, span.segment)?;
                self.nurbs_geometry(definition)?
                    .jet_on_span(span_index, parameter)?
            }
            definition @ (CurveDefinition::Ellipse { .. }
            | CurveDefinition::EllipticalArc { .. }
            | CurveDefinition::RationalQuadraticConic { .. }
            | CurveDefinition::ParabolaSegment { .. }
            | CurveDefinition::HyperbolaSegment { .. }) => self
                .conic_geometry(definition)
                .map_err(document_curve_conic_geometry_error)?
                .evaluate(parameter)?,
        })
    }

    /// Evaluates one finite point-valued feature from immutable persistent conic geometry.
    ///
    /// # Errors
    ///
    /// Returns a typed definition/evaluation failure or an unsupported feature/family pair.
    pub fn evaluate_conic_feature(
        &self,
        curve: CurveId,
        feature: DocumentConicFeature,
    ) -> Result<[f64; 2], DocumentConicQueryError> {
        let definition = &self
            .curve(curve)
            .ok_or_else(|| unknown("curve", curve.0))?
            .definition;
        if !is_conic_definition(definition) {
            return Err(DocumentConicQueryError::UnsupportedFeature { curve, feature });
        }
        let geometry = self
            .conic_geometry(definition)
            .map_err(document_query_conic_geometry_error)?;
        Self::evaluate_conic_feature_inner(curve, geometry, feature)
    }

    fn evaluate_conic_feature_inner(
        curve: CurveId,
        geometry: crate::ConicGeometry,
        feature: DocumentConicFeature,
    ) -> Result<[f64; 2], DocumentConicQueryError> {
        use crate::ConicGeometry as G;
        use DocumentConicFeature as F;
        let endpoint = |points: [geosolve_geometry::Point2<f64>; 2], endpoint| match endpoint {
            FeatureEndpoint::Start => points[0],
            FeatureEndpoint::End => points[1],
        };
        let point = match (geometry, feature) {
            (G::Ellipse(value), F::Center) => Some(value.center()),
            (G::EllipticalArc(value), F::Center) => Some(value.ellipse().center()),
            (G::HyperbolaSegment(value), F::Center) => Some(value.center()),
            (G::Ellipse(value), F::Focus { index }) => indexed_point(value.foci(), index),
            (G::EllipticalArc(value), F::Focus { index }) => {
                indexed_point(value.ellipse().foci(), index)
            }
            (G::HyperbolaSegment(value), F::Focus { index }) => indexed_point(value.foci(), index),
            (G::ParabolaSegment(value), F::Focus { index: 0 }) => Some(value.focus()),
            (G::Ellipse(value), F::MajorAxisEndpoint { endpoint: selected }) => {
                Some(endpoint(value.major_axis_endpoints(), selected))
            }
            (G::EllipticalArc(value), F::MajorAxisEndpoint { endpoint: selected }) => {
                Some(endpoint(value.ellipse().major_axis_endpoints(), selected))
            }
            (G::Ellipse(value), F::MinorAxisEndpoint { endpoint: selected }) => {
                Some(endpoint(value.minor_axis_endpoints(), selected))
            }
            (G::EllipticalArc(value), F::MinorAxisEndpoint { endpoint: selected }) => {
                Some(endpoint(value.ellipse().minor_axis_endpoints(), selected))
            }
            (G::EllipticalArc(value), F::BoundedEndpoint { endpoint: selected }) => Some(endpoint(
                [value.start_point()?, value.end_point()?],
                selected,
            )),
            (G::RationalQuadratic(value), F::BoundedEndpoint { endpoint: selected }) => {
                Some(endpoint([value.start_point(), value.end_point()], selected))
            }
            (G::ParabolaSegment(value), F::BoundedEndpoint { endpoint: selected }) => Some(
                endpoint([value.start_point()?, value.end_point()?], selected),
            ),
            (G::HyperbolaSegment(value), F::BoundedEndpoint { endpoint: selected }) => Some(
                endpoint([value.start_point()?, value.end_point()?], selected),
            ),
            (G::HyperbolaSegment(value), F::SelectedBranchVertex) => {
                Some(value.selected_branch_vertex())
            }
            _ => None,
        }
        .ok_or(DocumentConicQueryError::UnsupportedFeature { curve, feature })?;
        finite_query_point(curve, point)
    }

    /// Measures one finite CAD-useful scalar from immutable persistent conic geometry.
    ///
    /// # Errors
    ///
    /// Returns a typed definition failure or an unsupported measurement/family pair.
    pub fn measure_conic(
        &self,
        curve: CurveId,
        measurement: DocumentConicMeasurement,
    ) -> Result<f64, DocumentConicQueryError> {
        use crate::ConicGeometry as G;
        use DocumentConicMeasurement as M;
        let definition = &self
            .curve(curve)
            .ok_or_else(|| unknown("curve", curve.0))?
            .definition;
        if !is_conic_definition(definition) {
            return Err(DocumentConicQueryError::UnsupportedMeasurement { curve, measurement });
        }
        let geometry = self
            .conic_geometry(definition)
            .map_err(document_query_conic_geometry_error)?;
        let value = match (geometry, measurement) {
            (G::Ellipse(value), M::MajorAxisLength) => Some(value.major_axis_length()),
            (G::EllipticalArc(value), M::MajorAxisLength) => {
                Some(value.ellipse().major_axis_length())
            }
            (G::Ellipse(value), M::MinorAxisLength) => Some(value.minor_axis_length()),
            (G::EllipticalArc(value), M::MinorAxisLength) => {
                Some(value.ellipse().minor_axis_length())
            }
            (G::Ellipse(value), M::LinearEccentricity) => Some(value.linear_eccentricity()),
            (G::EllipticalArc(value), M::LinearEccentricity) => {
                Some(value.ellipse().linear_eccentricity())
            }
            (G::ParabolaSegment(value), M::FocalDistance) => Some(value.focal_length()),
            (G::HyperbolaSegment(value), M::FocalDistance) => Some(value.focal_distance()),
            (G::HyperbolaSegment(value), M::TransverseAxisLength) => {
                Some(value.transverse_axis_length())
            }
            (G::HyperbolaSegment(value), M::ConjugateAxisLength) => {
                Some(value.conjugate_axis_length())
            }
            _ => None,
        }
        .ok_or(DocumentConicQueryError::UnsupportedMeasurement { curve, measurement })?;
        if value.is_finite() {
            Ok(value)
        } else {
            Err(DocumentConicQueryError::NonFiniteResult { curve })
        }
    }

    /// Reports whether an ellipse's stored axis orientation is geometrically observable.
    ///
    /// # Errors
    ///
    /// Returns an unsupported-feature error for non-ellipse conics.
    pub fn conic_axis_observability(
        &self,
        curve: CurveId,
    ) -> Result<geosolve_geometry::EllipseAxisObservability, DocumentConicQueryError> {
        let definition = &self
            .curve(curve)
            .ok_or_else(|| unknown("curve", curve.0))?
            .definition;
        if !is_conic_definition(definition) {
            return Err(DocumentConicQueryError::UnsupportedFeature {
                curve,
                feature: DocumentConicFeature::MajorAxisEndpoint {
                    endpoint: FeatureEndpoint::Start,
                },
            });
        }
        self.conic_geometry(definition)
            .map_err(document_query_conic_geometry_error)?
            .axis_observability()
            .ok_or(DocumentConicQueryError::UnsupportedFeature {
                curve,
                feature: DocumentConicFeature::MajorAxisEndpoint {
                    endpoint: FeatureEndpoint::Start,
                },
            })
    }

    #[allow(clippy::too_many_lines)]
    fn conic_geometry(
        &self,
        definition: &CurveDefinition,
    ) -> Result<crate::ConicGeometry, DocumentConicGeometryError> {
        use geosolve_geometry::{
            DirectedParameterTrim, Ellipse2, EllipticalArc2, HyperbolaSegment2, ParabolaSegment2,
            Point2, RationalQuadraticConicSegment2, UnitDirection2, Vector2,
        };

        let point = |id: DesignPointId| -> Result<Point2<f64>, DocumentConicGeometryError> {
            let value = self.require_point(id)?;
            Ok(Point2::new(value.position[0], value.position[1]))
        };
        let axis = |first: Point2<f64>, second: Point2<f64>| {
            UnitDirection2::try_new(second - first).map_err(DocumentConicGeometryError::from)
        };
        Ok(match definition {
            CurveDefinition::Ellipse {
                center,
                major_axis_point,
                minor_axis_ratio,
            } => {
                let center = point(*center)?;
                let axis_point = point(*major_axis_point)?;
                let semi_major = (axis_point - center).x.hypot((axis_point - center).y);
                let ratio = self.require_scalar(*minor_axis_ratio)?.value;
                crate::ConicGeometry::Ellipse(Ellipse2::try_new(
                    center,
                    axis(center, axis_point)?,
                    semi_major,
                    semi_major * ratio,
                )?)
            }
            CurveDefinition::EllipticalArc {
                center,
                major_axis_point,
                minor_axis_ratio,
                start_angle,
                end_angle,
                sweep,
            } => {
                let center = point(*center)?;
                let axis_point = point(*major_axis_point)?;
                let semi_major = (axis_point - center).x.hypot((axis_point - center).y);
                let ratio = self.require_scalar(*minor_axis_ratio)?.value;
                let start = self.require_scalar(*start_angle)?.value;
                let end = self.require_scalar(*end_angle)?.value;
                let signed_sweep = document_arc_signed_sweep(start, end, *sweep)?;
                let ellipse = Ellipse2::try_new(
                    center,
                    axis(center, axis_point)?,
                    semi_major,
                    semi_major * ratio,
                )?;
                crate::ConicGeometry::EllipticalArc(EllipticalArc2::try_new(
                    ellipse,
                    start,
                    signed_sweep,
                )?)
            }
            CurveDefinition::RationalQuadraticConic {
                start,
                weighted_middle,
                middle_weight,
                end,
            } => crate::ConicGeometry::RationalQuadratic(
                RationalQuadraticConicSegment2::try_from_homogeneous_middle(
                    point(*start)?,
                    Vector2::new(weighted_middle[0], weighted_middle[1]),
                    self.require_scalar(*middle_weight)?.value,
                    point(*end)?,
                )?,
            ),
            CurveDefinition::ParabolaSegment {
                vertex,
                focus,
                trim_start,
                trim_end,
            } => {
                let vertex = point(*vertex)?;
                let focus = point(*focus)?;
                let focal_length = (focus - vertex).x.hypot((focus - vertex).y);
                crate::ConicGeometry::ParabolaSegment(ParabolaSegment2::try_new(
                    vertex,
                    axis(vertex, focus)?,
                    focal_length,
                    DirectedParameterTrim::try_new(
                        self.require_scalar(*trim_start)?.value,
                        self.require_scalar(*trim_end)?.value,
                    )?,
                )?)
            }
            CurveDefinition::HyperbolaSegment {
                center,
                transverse_axis_point,
                semi_conjugate,
                branch,
                trim_start,
                trim_end,
            } => {
                let center = point(*center)?;
                let axis_point = point(*transverse_axis_point)?;
                let semi_transverse = (axis_point - center).x.hypot((axis_point - center).y);
                crate::ConicGeometry::HyperbolaSegment(HyperbolaSegment2::try_new(
                    center,
                    axis(center, axis_point)?,
                    semi_transverse,
                    self.require_scalar(*semi_conjugate)?.value,
                    document_hyperbola_branch(*branch),
                    DirectedParameterTrim::try_new(
                        self.require_scalar(*trim_start)?.value,
                        self.require_scalar(*trim_end)?.value,
                    )?,
                )?)
            }
            _ => {
                return Err(DocumentError::InvalidField {
                    field: "curve",
                    message: "expected a persistent conic curve".into(),
                }
                .into());
            }
        })
    }

    #[must_use]
    pub fn constraint(&self, id: DocumentConstraintId) -> Option<&DocumentConstraint> {
        self.constraints.iter().find(|value| value.id == id)
    }

    #[must_use]
    pub fn dimension(&self, id: DocumentDimensionId) -> Option<&DocumentDimension> {
        self.dimensions.iter().find(|value| value.id == id)
    }

    /// Validates one semantic feature reference against this document.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing entity or unsupported feature selection.
    pub fn validate_feature(&self, feature: FeatureRef) -> Result<(), DocumentError> {
        match feature {
            FeatureRef::Point { point } => {
                self.require_point(point)?;
            }
            FeatureRef::CurveEndpoint { curve, .. } => {
                let value = self.curve(curve).ok_or_else(|| unknown("curve", curve.0))?;
                if matches!(
                    value.definition,
                    CurveDefinition::Circle { .. }
                        | CurveDefinition::Ellipse { .. }
                        | CurveDefinition::BSpline {
                            form: DocumentBSplineForm::Periodic,
                            ..
                        }
                        | CurveDefinition::Nurbs {
                            form: DocumentBSplineForm::Periodic,
                            ..
                        }
                ) {
                    return invalid("feature endpoint", "a periodic curve has no endpoint");
                }
            }
            FeatureRef::CurveAxis { curve } => {
                let value = self.curve(curve).ok_or_else(|| unknown("curve", curve.0))?;
                if !matches!(
                    value.definition,
                    CurveDefinition::Line { .. }
                        | CurveDefinition::Polyline { .. }
                        | CurveDefinition::Ellipse { .. }
                        | CurveDefinition::EllipticalArc { .. }
                        | CurveDefinition::ParabolaSegment { .. }
                        | CurveDefinition::HyperbolaSegment { .. }
                ) {
                    return invalid("feature axis", "curve family has no semantic axis");
                }
            }
            FeatureRef::CurveCenter { curve } => {
                let value = self.curve(curve).ok_or_else(|| unknown("curve", curve.0))?;
                if !matches!(
                    value.definition,
                    CurveDefinition::Circle { .. }
                        | CurveDefinition::CircularArc { .. }
                        | CurveDefinition::Ellipse { .. }
                        | CurveDefinition::EllipticalArc { .. }
                        | CurveDefinition::HyperbolaSegment { .. }
                ) {
                    return invalid("feature center", "curve family has no center");
                }
            }
            FeatureRef::CurveControl { curve, index } => {
                let entity = self.curve(curve).ok_or_else(|| unknown("curve", curve.0))?;
                let count = match &entity.definition {
                    CurveDefinition::Line { .. } => 2,
                    CurveDefinition::Polyline { points, .. } => points.len(),
                    CurveDefinition::Circle { .. }
                    | CurveDefinition::CircularArc { .. }
                    | CurveDefinition::Ellipse { .. }
                    | CurveDefinition::EllipticalArc { .. }
                    | CurveDefinition::RationalQuadraticConic { .. }
                    | CurveDefinition::ParabolaSegment { .. }
                    | CurveDefinition::HyperbolaSegment { .. }
                    | CurveDefinition::BSpline { .. }
                    | CurveDefinition::Nurbs { .. } => 0,
                    CurveDefinition::QuadraticBezier { .. } => 3,
                    CurveDefinition::CubicBezier { .. } => 4,
                };
                if usize::try_from(index).map_or(true, |value| value >= count) {
                    return invalid("feature control", "control index is outside the curve");
                }
            }
            FeatureRef::CurveFocus { curve, index } => {
                let value = self.curve(curve).ok_or_else(|| unknown("curve", curve.0))?;
                let count = match value.definition {
                    CurveDefinition::Ellipse { .. }
                    | CurveDefinition::EllipticalArc { .. }
                    | CurveDefinition::HyperbolaSegment { .. } => 2,
                    CurveDefinition::ParabolaSegment { .. } => 1,
                    _ => 0,
                };
                if index >= count {
                    return invalid("feature focus", "focus index is outside the curve");
                }
            }
            FeatureRef::FixedCurveLocation { contact } => {
                self.require_contact(contact)?;
            }
        }
        Ok(())
    }

    /// Adds a finite point and allocates a never-reused persistent ID.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid label or non-finite position.
    pub fn add_point(
        &mut self,
        label: impl Into<String>,
        position: [f64; 2],
    ) -> Result<DesignPointId, DocumentError> {
        self.add_named_point(label, position)
    }

    /// Adds a finite point with the exact supplied label.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid label or non-finite position.
    pub fn add_named_point(
        &mut self,
        label: impl Into<String>,
        position: [f64; 2],
    ) -> Result<DesignPointId, DocumentError> {
        let label = label.into();
        validate_label(&label, "point label")?;
        finite_pair(position, "point position")?;
        let id = DesignPointId(self.allocate_id()?);
        self.points.push(DesignPoint {
            id,
            label,
            position,
        });
        Ok(id)
    }

    /// Adds one typed scalar.
    ///
    /// # Errors
    ///
    /// Returns an error when the label, value, or domain is invalid.
    pub fn add_scalar(
        &mut self,
        label: impl Into<String>,
        value: f64,
        unit: ScalarUnit,
        domain: ScalarDomain,
    ) -> Result<DesignScalarId, DocumentError> {
        let label = label.into();
        validate_label(&label, "scalar label")?;
        validate_scalar_value(value, domain)?;
        let id = DesignScalarId(self.allocate_id()?);
        self.scalars.push(DesignScalar {
            id,
            label,
            value,
            unit,
            domain,
        });
        Ok(id)
    }

    /// Adds a line, polyline, circle, or arc after full reference validation.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid data or a missing semantic reference.
    pub fn add_curve(
        &mut self,
        label: impl Into<String>,
        definition: CurveDefinition,
    ) -> Result<CurveId, DocumentError> {
        let label = label.into();
        validate_label(&label, "curve label")?;
        let id = CurveId(self.allocate_id()?);
        self.curves.push(DesignCurve {
            id,
            label,
            definition,
        });
        if let Err(error) = self.validate() {
            self.curves.pop();
            return Err(error);
        }
        Ok(id)
    }

    /// Inserts one B-spline knot while preserving the parameterized curve.
    ///
    /// The edit retains every existing control point ID, allocates one new control
    /// point, and remaps contacts on a split span atomically.
    ///
    /// # Errors
    ///
    /// Rejects a missing/non-spline curve, invalid insertion, exhausted identity,
    /// or a refined document that fails complete validation.
    #[allow(clippy::too_many_lines)]
    pub fn insert_bspline_knot(
        &mut self,
        curve: CurveId,
        parameter: f64,
    ) -> Result<DocumentBSplineInsertion, DocumentError> {
        let mut candidate = self.clone();
        let curve_value = candidate
            .curve(curve)
            .ok_or_else(|| unknown("curve", curve.0))?
            .clone();
        let CurveDefinition::BSpline {
            controls: old_controls,
            span_ids: old_span_ids,
            next_span_id,
            ..
        } = &curve_value.definition
        else {
            return invalid("curve", "knot insertion requires a B-spline");
        };
        let geometry = candidate
            .bspline_geometry(&curve_value.definition)
            .map_err(|error| document_bspline_curve_error(curve, error))?;
        let refinement = geometry
            .insert_knot(parameter)
            .map_err(|source| DocumentError::BSplineInsertion { curve, source })?;
        let refined_positions = refinement.curve().controls().to_vec();
        let stencils = refinement.control_stencils();

        let mut output_ids = vec![None; stencils.len()];
        let mut copied_controls = BTreeSet::new();
        for (output, stencil) in stencils.iter().enumerate() {
            if stencil.second_control.is_none() {
                if !copied_controls.insert(stencil.first_control) {
                    return invalid(
                        "curve.controls",
                        "refinement duplicated an exact control identity",
                    );
                }
                output_ids[output] = Some(old_controls[stencil.first_control]);
            }
        }
        let remaining_controls = (0..old_controls.len())
            .filter(|control| !copied_controls.contains(control))
            .collect::<Vec<_>>();
        let blended_outputs = stencils
            .iter()
            .enumerate()
            .filter_map(|(output, stencil)| stencil.second_control.map(|_| output))
            .collect::<Vec<_>>();
        if blended_outputs.len() != remaining_controls.len() + 1 {
            return invalid(
                "curve.controls",
                "refinement control provenance is inconsistent",
            );
        }
        for (output, old_control) in blended_outputs
            .iter()
            .copied()
            .zip(remaining_controls.iter().copied())
        {
            output_ids[output] = Some(old_controls[old_control]);
        }
        let fresh_output = *blended_outputs
            .last()
            .ok_or_else(|| DocumentError::InvalidField {
                field: "curve.controls",
                message: "knot insertion did not create a control coefficient".into(),
            })?;
        let new_control = candidate.add_point(
            format!("{} inserted control", curve_value.label),
            [
                refined_positions[fresh_output].x,
                refined_positions[fresh_output].y,
            ],
        )?;
        output_ids[fresh_output] = Some(new_control);
        let output_ids = output_ids
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| DocumentError::InvalidField {
                field: "curve.controls",
                message: "refinement left an unassigned control identity".into(),
            })?;
        for (control, position) in output_ids.iter().zip(&refined_positions) {
            candidate
                .point_mut(*control)
                .ok_or_else(|| unknown("point", control.0))?
                .position = [position.x, position.y];
        }

        let split_ordinal = refinement
            .split_span()
            .map(|span| usize::try_from(span.ordinal()))
            .transpose()
            .map_err(|_| DocumentError::InvalidField {
                field: "curve.span_ids",
                message: "split span ordinal is unrepresentable".into(),
            })?;
        let (new_span_id, retained_span, old_interval) = if let Some(ordinal) = split_ordinal {
            let retained =
                *old_span_ids
                    .get(ordinal)
                    .ok_or_else(|| DocumentError::InvalidField {
                        field: "curve.span_ids",
                        message: "split span has no semantic identity".into(),
                    })?;
            let interval = geometry
                .basis()
                .spans()
                .get(ordinal)
                .cloned()
                .ok_or_else(|| DocumentError::InvalidField {
                    field: "curve.span_ids",
                    message: "split span has no numerical interval".into(),
                })?;
            (Some(*next_span_id), Some(retained), Some(interval))
        } else {
            (None, None, None)
        };

        let mut refined_span_ids = old_span_ids.clone();
        let mut refined_next_span_id = *next_span_id;
        if let (Some(ordinal), Some(allocated)) = (split_ordinal, new_span_id) {
            refined_span_ids.insert(ordinal + 1, allocated);
            refined_next_span_id = allocated.checked_add(1).ok_or(DocumentError::IdExhausted)?;
        }
        let CurveDefinition::BSpline {
            controls,
            knots,
            span_ids,
            next_span_id,
            ..
        } = &mut candidate
            .curve_mut(curve)
            .ok_or_else(|| unknown("curve", curve.0))?
            .definition
        else {
            return invalid("curve", "knot insertion curve family changed");
        };
        *controls = output_ids;
        *knots = refinement.curve().basis().knots().to_vec();
        *span_ids = refined_span_ids;
        *next_span_id = refined_next_span_id;

        let normalized_insertion = match geometry.basis().parameter_domain() {
            geosolve_geometry::CurveParameterDomain::Periodic { period } => {
                parameter.rem_euclid(period)
            }
            geosolve_geometry::CurveParameterDomain::Bounded { .. } => parameter,
            geosolve_geometry::CurveParameterDomain::SupportingLine => {
                unreachable!("a B-spline never has a supporting-line domain")
            }
        };
        let mut migrated_contacts = Vec::new();
        if let (Some(retained), Some(right), Some(interval)) =
            (retained_span, new_span_id, old_interval)
        {
            let contact_ids = candidate
                .contacts
                .iter()
                .filter(|contact| contact.curve.curve == curve && contact.curve.segment == retained)
                .map(|contact| contact.id)
                .collect::<Vec<_>>();
            for contact_id in contact_ids {
                let contact = candidate.require_contact(contact_id)?.clone();
                let local = candidate.require_scalar(contact.parameter)?.value;
                let width = interval.upper() - interval.lower();
                let native = if local.to_bits() == 0.0f64.to_bits() {
                    interval.lower()
                } else if local.to_bits() == 1.0f64.to_bits() {
                    interval.upper()
                } else {
                    width.mul_add(local, interval.lower())
                };
                let left_child = native <= normalized_insertion;
                let (semantic, remapped) = if left_child {
                    (
                        retained,
                        (native - interval.lower()) / (normalized_insertion - interval.lower()),
                    )
                } else {
                    (
                        right,
                        (native - normalized_insertion) / (interval.upper() - normalized_insertion),
                    )
                };
                if !remapped.is_finite() {
                    return invalid(
                        "contact.parameter",
                        "knot insertion produced a non-finite contact coordinate",
                    );
                }
                let remapped = remapped.clamp(0.0, 1.0);
                let neighborhood = remap_bspline_contact_neighborhood(
                    contact.neighborhood,
                    &interval,
                    normalized_insertion,
                    left_child,
                    remapped,
                )?;
                candidate
                    .scalar_mut(contact.parameter)
                    .ok_or_else(|| unknown("scalar", contact.parameter.0))?
                    .value = remapped;
                let contact = candidate
                    .contact_mut(contact_id)
                    .ok_or_else(|| unknown("contact", contact_id.0))?;
                contact.curve.segment = semantic;
                contact.neighborhood = neighborhood;
                migrated_contacts.push(contact_id);
            }
        }
        candidate.validate()?;
        *self = candidate;
        Ok(DocumentBSplineInsertion {
            curve,
            new_control,
            new_span_id,
            migrated_contacts,
        })
    }

    /// Inserts one NURBS knot by immutable homogeneous refinement.
    ///
    /// Every old control and weight identity survives, one fresh control/weight
    /// pair is allocated, and the selected gauge identity is normalized back to
    /// exact one before the candidate is validated.
    ///
    /// # Errors
    ///
    /// Rejects a missing/non-NURBS curve, invalid homogeneous refinement,
    /// exhausted identity, or an invalid refined document.
    #[allow(clippy::too_many_lines)]
    pub fn insert_nurbs_knot(
        &mut self,
        curve: CurveId,
        parameter: f64,
    ) -> Result<DocumentNurbsInsertion, DocumentError> {
        let mut candidate = self.clone();
        let curve_value = candidate
            .curve(curve)
            .ok_or_else(|| unknown("curve", curve.0))?
            .clone();
        let CurveDefinition::Nurbs {
            controls: old_controls,
            weights: old_weights,
            gauge_weight,
            span_ids: old_span_ids,
            next_span_id,
            ..
        } = &curve_value.definition
        else {
            return invalid("curve", "knot insertion requires a NURBS curve");
        };
        let geometry = candidate
            .nurbs_geometry(&curve_value.definition)
            .map_err(|error| document_nurbs_curve_error(curve, error))?;
        let refinement = geometry
            .insert_knot(parameter)
            .map_err(|source| DocumentError::NurbsInsertion { curve, source })?;
        let refined_positions = refinement.curve().controls().to_vec();
        let refined_weights = refinement.curve().weights().to_vec();
        let provenance = refinement.control_provenance();

        let mut output_ids = vec![None; provenance.len()];
        let mut copied_controls = BTreeSet::new();
        for (output, source) in provenance.iter().enumerate() {
            if let geosolve_geometry::NurbsControlProvenance::Copy { control } = source {
                if !copied_controls.insert(*control) {
                    return invalid(
                        "curve.controls",
                        "refinement duplicated an exact NURBS control identity",
                    );
                }
                output_ids[output] = Some((old_controls[*control], old_weights[*control]));
            }
        }
        let remaining_controls = (0..old_controls.len())
            .filter(|control| !copied_controls.contains(control))
            .collect::<Vec<_>>();
        let blended_outputs = provenance
            .iter()
            .enumerate()
            .filter_map(|(output, source)| {
                matches!(
                    source,
                    geosolve_geometry::NurbsControlProvenance::Blend { .. }
                )
                .then_some(output)
            })
            .collect::<Vec<_>>();
        if blended_outputs.len() != remaining_controls.len() + 1 {
            return invalid(
                "curve.controls",
                "NURBS refinement control provenance is inconsistent",
            );
        }
        for (output, old_control) in blended_outputs
            .iter()
            .copied()
            .zip(remaining_controls.iter().copied())
        {
            output_ids[output] = Some((old_controls[old_control], old_weights[old_control]));
        }
        let fresh_output = *blended_outputs
            .last()
            .ok_or_else(|| DocumentError::InvalidField {
                field: "curve.controls",
                message: "NURBS knot insertion did not create a control coefficient".into(),
            })?;
        let gauge_output = output_ids
            .iter()
            .position(|ids| ids.is_some_and(|(_, weight)| weight == *gauge_weight))
            .ok_or_else(|| DocumentError::InvalidField {
                field: "curve.gauge_weight",
                message: "NURBS refinement lost the selected gauge identity".into(),
            })?;
        let gauge_scale = refined_weights[gauge_output];
        finite_positive(gauge_scale, "refined NURBS gauge weight")?;
        let normalized_weights = refined_weights
            .iter()
            .map(|weight| {
                let normalized = weight / gauge_scale;
                finite_positive(normalized, "refined NURBS weight")?;
                Ok(normalized)
            })
            .collect::<Result<Vec<_>, DocumentError>>()?;

        let new_control = candidate.add_point(
            format!("{} inserted control", curve_value.label),
            [
                refined_positions[fresh_output].x,
                refined_positions[fresh_output].y,
            ],
        )?;
        let new_weight = candidate.add_scalar(
            format!("{} inserted weight", curve_value.label),
            normalized_weights[fresh_output],
            ScalarUnit::Parameter,
            ScalarDomain::Positive,
        )?;
        output_ids[fresh_output] = Some((new_control, new_weight));
        let output_ids = output_ids
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| DocumentError::InvalidField {
                field: "curve.controls",
                message: "NURBS refinement left an unassigned identity".into(),
            })?;
        for (((control, weight), position), value) in output_ids
            .iter()
            .zip(&refined_positions)
            .zip(&normalized_weights)
        {
            candidate
                .point_mut(*control)
                .ok_or_else(|| unknown("point", control.0))?
                .position = [position.x, position.y];
            candidate
                .scalar_mut(*weight)
                .ok_or_else(|| unknown("scalar", weight.0))?
                .value = if *weight == *gauge_weight {
                1.0
            } else {
                *value
            };
        }

        let split_ordinal = refinement
            .split_span()
            .map(|span| usize::try_from(span.ordinal()))
            .transpose()
            .map_err(|_| DocumentError::InvalidField {
                field: "curve.span_ids",
                message: "split span ordinal is unrepresentable".into(),
            })?;
        let (new_span_id, retained_span, old_interval) = if let Some(ordinal) = split_ordinal {
            let retained =
                *old_span_ids
                    .get(ordinal)
                    .ok_or_else(|| DocumentError::InvalidField {
                        field: "curve.span_ids",
                        message: "split span has no semantic identity".into(),
                    })?;
            let interval = geometry
                .basis()
                .spans()
                .get(ordinal)
                .cloned()
                .ok_or_else(|| DocumentError::InvalidField {
                    field: "curve.span_ids",
                    message: "split span has no numerical interval".into(),
                })?;
            (Some(*next_span_id), Some(retained), Some(interval))
        } else {
            (None, None, None)
        };
        let mut refined_span_ids = old_span_ids.clone();
        let mut refined_next_span_id = *next_span_id;
        if let (Some(ordinal), Some(allocated)) = (split_ordinal, new_span_id) {
            refined_span_ids.insert(ordinal + 1, allocated);
            refined_next_span_id = allocated.checked_add(1).ok_or(DocumentError::IdExhausted)?;
        }
        let CurveDefinition::Nurbs {
            controls,
            weights,
            knots,
            span_ids,
            next_span_id,
            ..
        } = &mut candidate
            .curve_mut(curve)
            .ok_or_else(|| unknown("curve", curve.0))?
            .definition
        else {
            return invalid("curve", "knot insertion curve family changed");
        };
        *controls = output_ids.iter().map(|(control, _)| *control).collect();
        *weights = output_ids.iter().map(|(_, weight)| *weight).collect();
        *knots = refinement.curve().basis().knots().to_vec();
        *span_ids = refined_span_ids;
        *next_span_id = refined_next_span_id;

        let normalized_insertion = match geometry.basis().parameter_domain() {
            geosolve_geometry::CurveParameterDomain::Periodic { period } => {
                parameter.rem_euclid(period)
            }
            geosolve_geometry::CurveParameterDomain::Bounded { .. } => parameter,
            geosolve_geometry::CurveParameterDomain::SupportingLine => {
                unreachable!("a NURBS never has a supporting-line domain")
            }
        };
        let migrated_contacts = if let (Some(retained), Some(right), Some(interval)) =
            (retained_span, new_span_id, old_interval)
        {
            candidate.migrate_split_spline_contacts(
                curve,
                retained,
                right,
                &interval,
                normalized_insertion,
            )?
        } else {
            Vec::new()
        };
        candidate.validate()?;
        *self = candidate;
        Ok(DocumentNurbsInsertion {
            curve,
            new_control,
            new_weight,
            new_span_id,
            migrated_contacts,
        })
    }

    fn migrate_split_spline_contacts(
        &mut self,
        curve: CurveId,
        retained_span: u32,
        right_span: u32,
        old_interval: &geosolve_geometry::BSplineSpan,
        inserted_knot: f64,
    ) -> Result<Vec<ContactId>, DocumentError> {
        let contact_ids = self
            .contacts
            .iter()
            .filter(|contact| {
                contact.curve.curve == curve && contact.curve.segment == retained_span
            })
            .map(|contact| contact.id)
            .collect::<Vec<_>>();
        let mut migrated = Vec::with_capacity(contact_ids.len());
        for contact_id in contact_ids {
            let contact = self.require_contact(contact_id)?.clone();
            let local = self.require_scalar(contact.parameter)?.value;
            let width = old_interval.upper() - old_interval.lower();
            let native = if local.to_bits() == 0.0f64.to_bits() {
                old_interval.lower()
            } else if local.to_bits() == 1.0f64.to_bits() {
                old_interval.upper()
            } else {
                width.mul_add(local, old_interval.lower())
            };
            let left_child = native <= inserted_knot;
            let (semantic, remapped) = if left_child {
                (
                    retained_span,
                    (native - old_interval.lower()) / (inserted_knot - old_interval.lower()),
                )
            } else {
                (
                    right_span,
                    (native - inserted_knot) / (old_interval.upper() - inserted_knot),
                )
            };
            if !remapped.is_finite() {
                return invalid(
                    "contact.parameter",
                    "knot insertion produced a non-finite contact coordinate",
                );
            }
            let remapped = remapped.clamp(0.0, 1.0);
            let neighborhood = remap_bspline_contact_neighborhood(
                contact.neighborhood,
                old_interval,
                inserted_knot,
                left_child,
                remapped,
            )?;
            self.scalar_mut(contact.parameter)
                .ok_or_else(|| unknown("scalar", contact.parameter.0))?
                .value = remapped;
            let contact = self
                .contact_mut(contact_id)
                .ok_or_else(|| unknown("contact", contact_id.0))?;
            contact.curve.segment = semantic;
            contact.neighborhood = neighborhood;
            migrated.push(contact_id);
        }
        Ok(migrated)
    }

    /// Moves an endpoint contact to the explicit adjacent B-spline span.
    ///
    /// Periodic seam transitions update winding by one. Tangent-bearing contacts
    /// require topology to guarantee `C1` at the crossed knot.
    ///
    /// # Errors
    ///
    /// Rejects a non-spline/non-endpoint contact, unavailable clamped neighbor,
    /// insufficient continuity, winding overflow, or invalid resulting document.
    #[allow(clippy::too_many_lines)]
    pub fn transition_bspline_contact(
        &mut self,
        contact: ContactId,
        direction: DocumentBSplineSpanDirection,
    ) -> Result<(), DocumentError> {
        let mut candidate = self.clone();
        let contact_value = candidate.require_contact(contact)?.clone();
        let curve_value = candidate
            .curve(contact_value.curve.curve)
            .ok_or_else(|| unknown("curve", contact_value.curve.curve.0))?
            .clone();
        let CurveDefinition::BSpline { form, span_ids, .. } = &curve_value.definition else {
            return invalid("contact.curve", "span transition requires a B-spline");
        };
        let ordinal = span_ids
            .iter()
            .position(|span| *span == contact_value.curve.segment)
            .ok_or_else(|| DocumentError::InvalidField {
                field: "contact.curve",
                message: "contact semantic span is stale".into(),
            })?;
        let scalar = candidate.require_scalar(contact_value.parameter)?.value;
        let basis = Self::spline_basis(&curve_value.definition)
            .map_err(|error| document_bspline_curve_error(contact_value.curve.curve, error))?;
        let span = basis
            .spans()
            .get(ordinal)
            .ok_or_else(|| DocumentError::InvalidField {
                field: "contact.curve",
                message: "contact semantic span mapping is inconsistent".into(),
            })?;
        let (target_ordinal, target_parameter, neighborhood, crossed_knot, winding_delta) =
            match direction {
                DocumentBSplineSpanDirection::Next => {
                    if scalar.to_bits() != 1.0f64.to_bits()
                        || contact_value.neighborhood != ContactNeighborhood::End
                    {
                        return invalid(
                            "contact.neighborhood",
                            "next-span transition requires the selected span end",
                        );
                    }
                    if ordinal + 1 < span_ids.len() {
                        (
                            ordinal + 1,
                            0.0,
                            ContactNeighborhood::Start,
                            span.upper(),
                            0,
                        )
                    } else if *form == DocumentBSplineForm::Periodic {
                        (0, 0.0, ContactNeighborhood::Start, span.upper(), 1)
                    } else {
                        return invalid("contact.curve", "clamped B-spline has no next span");
                    }
                }
                DocumentBSplineSpanDirection::Previous => {
                    if scalar.to_bits() != 0.0f64.to_bits()
                        || contact_value.neighborhood != ContactNeighborhood::Start
                    {
                        return invalid(
                            "contact.neighborhood",
                            "previous-span transition requires the selected span start",
                        );
                    }
                    if ordinal > 0 {
                        (ordinal - 1, 1.0, ContactNeighborhood::End, span.lower(), 0)
                    } else if *form == DocumentBSplineForm::Periodic {
                        (
                            span_ids.len() - 1,
                            1.0,
                            ContactNeighborhood::End,
                            span.lower(),
                            -1,
                        )
                    } else {
                        return invalid("contact.curve", "clamped B-spline has no previous span");
                    }
                }
            };
        let required_continuity = candidate.required_contact_continuity(&contact_value);
        basis
            .require_continuity(crossed_knot, required_continuity)
            .map_err(|source| DocumentError::BSplineEvaluation {
                curve: contact_value.curve.curve,
                source,
            })?;
        let winding = contact_value
            .winding
            .checked_add(winding_delta)
            .ok_or_else(|| DocumentError::InvalidField {
                field: "contact.winding",
                message: "periodic span transition exceeds i32 winding range".into(),
            })?;
        candidate
            .scalar_mut(contact_value.parameter)
            .ok_or_else(|| unknown("scalar", contact_value.parameter.0))?
            .value = target_parameter;
        let target = candidate
            .contact_mut(contact)
            .ok_or_else(|| unknown("contact", contact.0))?;
        target.curve.segment = span_ids[target_ordinal];
        target.winding = winding;
        target.neighborhood = neighborhood;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Moves an endpoint contact to the explicit adjacent NURBS span.
    ///
    /// Periodic seam transitions update winding by one. Tangent-bearing contacts
    /// require topology to guarantee `C1` at the crossed knot.
    ///
    /// # Errors
    ///
    /// Rejects a non-NURBS/non-endpoint contact, unavailable clamped neighbor,
    /// insufficient continuity, winding overflow, or invalid resulting document.
    #[allow(clippy::too_many_lines)]
    pub fn transition_nurbs_contact(
        &mut self,
        contact: ContactId,
        direction: DocumentBSplineSpanDirection,
    ) -> Result<(), DocumentError> {
        let mut candidate = self.clone();
        let contact_value = candidate.require_contact(contact)?.clone();
        let curve_value = candidate
            .curve(contact_value.curve.curve)
            .ok_or_else(|| unknown("curve", contact_value.curve.curve.0))?
            .clone();
        let CurveDefinition::Nurbs { form, span_ids, .. } = &curve_value.definition else {
            return invalid("contact.curve", "span transition requires a NURBS curve");
        };
        let ordinal = span_ids
            .iter()
            .position(|span| *span == contact_value.curve.segment)
            .ok_or_else(|| DocumentError::InvalidField {
                field: "contact.curve",
                message: "contact semantic span is stale".into(),
            })?;
        let scalar = candidate.require_scalar(contact_value.parameter)?.value;
        let basis = Self::spline_basis(&curve_value.definition)
            .map_err(|error| document_nurbs_curve_error(contact_value.curve.curve, error))?;
        let span = basis
            .spans()
            .get(ordinal)
            .ok_or_else(|| DocumentError::InvalidField {
                field: "contact.curve",
                message: "contact semantic span mapping is inconsistent".into(),
            })?;
        let (target_ordinal, target_parameter, neighborhood, crossed_knot, winding_delta) =
            match direction {
                DocumentBSplineSpanDirection::Next => {
                    if scalar.to_bits() != 1.0f64.to_bits()
                        || contact_value.neighborhood != ContactNeighborhood::End
                    {
                        return invalid(
                            "contact.neighborhood",
                            "next-span transition requires the selected span end",
                        );
                    }
                    if ordinal + 1 < span_ids.len() {
                        (
                            ordinal + 1,
                            0.0,
                            ContactNeighborhood::Start,
                            span.upper(),
                            0,
                        )
                    } else if *form == DocumentBSplineForm::Periodic {
                        (0, 0.0, ContactNeighborhood::Start, span.upper(), 1)
                    } else {
                        return invalid("contact.curve", "clamped NURBS has no next span");
                    }
                }
                DocumentBSplineSpanDirection::Previous => {
                    if scalar.to_bits() != 0.0f64.to_bits()
                        || contact_value.neighborhood != ContactNeighborhood::Start
                    {
                        return invalid(
                            "contact.neighborhood",
                            "previous-span transition requires the selected span start",
                        );
                    }
                    if ordinal > 0 {
                        (ordinal - 1, 1.0, ContactNeighborhood::End, span.lower(), 0)
                    } else if *form == DocumentBSplineForm::Periodic {
                        (
                            span_ids.len() - 1,
                            1.0,
                            ContactNeighborhood::End,
                            span.lower(),
                            -1,
                        )
                    } else {
                        return invalid("contact.curve", "clamped NURBS has no previous span");
                    }
                }
            };
        let required_continuity = candidate.required_contact_continuity(&contact_value);
        basis
            .require_continuity(crossed_knot, required_continuity)
            .map_err(|source| DocumentError::NurbsEvaluation {
                curve: contact_value.curve.curve,
                source: source.into(),
            })?;
        let winding = contact_value
            .winding
            .checked_add(winding_delta)
            .ok_or_else(|| DocumentError::InvalidField {
                field: "contact.winding",
                message: "periodic span transition exceeds i32 winding range".into(),
            })?;
        candidate
            .scalar_mut(contact_value.parameter)
            .ok_or_else(|| unknown("scalar", contact_value.parameter.0))?
            .value = target_parameter;
        let target = candidate
            .contact_mut(contact)
            .ok_or_else(|| unknown("contact", contact.0))?;
        target.curve.segment = span_ids[target_ordinal];
        target.winding = winding;
        target.neighborhood = neighborhood;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    fn required_contact_continuity(&self, contact: &ContactSlot) -> u32 {
        let mut required = u32::from(contact.tangent_orientation.is_some());
        for constraint in &self.constraints {
            let consumed = match &constraint.definition {
                DocumentConstraintDefinition::CurveDirection { curve_contact, .. }
                    if *curve_contact == contact.id =>
                {
                    1
                }
                DocumentConstraintDefinition::EqualCurvature {
                    first_contact,
                    second_contact,
                    ..
                } if *first_contact == contact.id || *second_contact == contact.id => 2,
                DocumentConstraintDefinition::EndpointContinuity {
                    first_contact,
                    second_contact,
                    continuity,
                } if *first_contact == contact.id || *second_contact == contact.id => {
                    match continuity {
                        DocumentCurveContinuity::G0 => 0,
                        DocumentCurveContinuity::G1 => 1,
                        DocumentCurveContinuity::G2
                        | DocumentCurveContinuity::ParametricC2 { .. } => 2,
                    }
                }
                _ => 0,
            };
            required = required.max(consumed);
        }
        required
    }

    /// Adds a semantic contact slot.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid contact state or a missing semantic reference.
    pub fn add_contact(
        &mut self,
        label: impl Into<String>,
        definition: ContactDefinition,
    ) -> Result<ContactId, DocumentError> {
        let label = label.into();
        validate_label(&label, "contact label")?;
        let id = ContactId(self.allocate_id()?);
        self.contacts.push(ContactSlot {
            id,
            label,
            curve: definition.curve,
            parameter: definition.parameter,
            domain: definition.domain,
            winding: definition.winding,
            neighborhood: definition.neighborhood,
            tangent_orientation: definition.tangent_orientation,
        });
        if let Err(error) = self.validate() {
            self.contacts.pop();
            return Err(error);
        }
        Ok(id)
    }

    /// Atomically creates a parameter scalar and contact slot for one alpha curve span.
    ///
    /// Circles and full ellipses use an angular periodic scalar; every other curve span uses a
    /// bounded `[0, 1]` parameter. Neighborhood, winding, and tangent orientation remain explicit.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid span, parameter, or incompatible explicit contact state.
    pub fn add_curve_contact(
        &mut self,
        label: impl Into<String>,
        curve: CurveSpan,
        parameter: f64,
        winding: i32,
        neighborhood: ContactNeighborhood,
        tangent_orientation: Option<TangentOrientation>,
    ) -> Result<ContactId, DocumentError> {
        let label = label.into();
        let definition = self
            .curve(curve.curve)
            .ok_or_else(|| unknown("curve", curve.curve.0))?
            .definition
            .clone();
        self.validate_span(curve)?;
        let (unit, scalar_domain, contact_domain) = if matches!(
            definition,
            CurveDefinition::Circle { .. } | CurveDefinition::Ellipse { .. }
        ) {
            (
                ScalarUnit::Angle,
                ScalarDomain::Periodic {
                    period: std::f64::consts::TAU,
                },
                ContactDomain::Periodic {
                    period: std::f64::consts::TAU,
                },
            )
        } else {
            (
                ScalarUnit::Parameter,
                ScalarDomain::Bounded {
                    lower: 0.0,
                    upper: 1.0,
                },
                ContactDomain::Bounded {
                    lower: 0.0,
                    upper: 1.0,
                },
            )
        };
        let mut candidate = self.clone();
        let scalar =
            candidate.add_scalar(format!("{label} parameter"), parameter, unit, scalar_domain)?;
        let contact = candidate.add_contact(
            label,
            ContactDefinition {
                curve,
                parameter: scalar,
                domain: contact_domain,
                winding,
                neighborhood,
                tangent_orientation,
            },
        )?;
        *self = candidate;
        Ok(contact)
    }

    /// Returns the ordinary explicit neighborhood for a picked alpha-curve parameter.
    ///
    /// Periodic circles use `Interior`; bounded endpoints use `Start`/`End`; other bounded
    /// picks use a local interval around the selected parameter.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid curve span or non-finite/out-of-domain parameter.
    pub fn picked_contact_neighborhood(
        &self,
        curve: CurveSpan,
        parameter: f64,
    ) -> Result<ContactNeighborhood, DocumentError> {
        let definition = &self.validate_span(curve)?.definition;
        if matches!(
            definition,
            CurveDefinition::Circle { .. } | CurveDefinition::Ellipse { .. }
        ) {
            finite(parameter, "contact parameter")?;
            return Ok(ContactNeighborhood::Interior);
        }
        if !parameter.is_finite() || !(0.0..=1.0).contains(&parameter) {
            return invalid("contact parameter", "must be within [0, 1]");
        }
        Ok(if parameter <= 1.0e-6 {
            ContactNeighborhood::Start
        } else if parameter >= 1.0 - 1.0e-6 {
            ContactNeighborhood::End
        } else {
            ContactNeighborhood::Local {
                lower: (parameter - 0.2).max(0.0),
                upper: (parameter + 0.2).min(1.0),
            }
        })
    }

    /// Adds one ordered geometric source.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid data or a missing semantic reference.
    pub fn add_constraint(
        &mut self,
        label: impl Into<String>,
        definition: DocumentConstraintDefinition,
    ) -> Result<DocumentConstraintId, DocumentError> {
        let label = label.into();
        validate_label(&label, "constraint label")?;
        let id = DocumentConstraintId(self.allocate_id()?);
        let source_id = DocumentSourceId(self.allocate_id()?);
        self.constraints.push(DocumentConstraint {
            id,
            source_id,
            label,
            suppressed: false,
            definition,
        });
        self.source_order.push(source_id);
        if let Err(error) = self.validate() {
            self.source_order.pop();
            self.constraints.pop();
            return Err(error);
        }
        Ok(id)
    }

    /// Adds one ordered driving or reference dimension source.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid data or a missing semantic reference.
    pub fn add_dimension(
        &mut self,
        label: impl Into<String>,
        definition: DocumentDimensionDefinition,
        mode: DocumentDimensionMode,
    ) -> Result<DocumentDimensionId, DocumentError> {
        let label = label.into();
        validate_label(&label, "dimension label")?;
        let id = DocumentDimensionId(self.allocate_id()?);
        let source_id = DocumentSourceId(self.allocate_id()?);
        self.dimensions.push(DocumentDimension {
            id,
            source_id,
            label,
            mode,
            suppressed: false,
            definition,
        });
        self.source_order.push(source_id);
        if let Err(error) = self.validate() {
            self.source_order.pop();
            self.dimensions.pop();
            return Err(error);
        }
        Ok(id)
    }

    /// Expands a rectangle into ordinary shared-corner geometry and sources.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid dimensions or any failed expanded edit.
    pub fn add_rectangle(
        &mut self,
        label: &str,
        origin: [f64; 2],
        width: f64,
        height: f64,
    ) -> Result<RectangleIds, DocumentError> {
        validate_label(label, "rectangle label")?;
        finite_pair(origin, "rectangle origin")?;
        finite_positive(width, "rectangle width")?;
        finite_positive(height, "rectangle height")?;
        let before = self.clone();
        let result = self.add_rectangle_inner(label, origin, width, height);
        if result.is_err() {
            let next_id = self.next_id;
            *self = before;
            self.next_id = next_id;
        }
        result
    }

    fn add_rectangle_inner(
        &mut self,
        label: &str,
        origin: [f64; 2],
        width: f64,
        height: f64,
    ) -> Result<RectangleIds, DocumentError> {
        let [x, y] = origin;
        let points = [
            self.add_named_point(format!("{label}.bottom_left"), [x, y])?,
            self.add_named_point(format!("{label}.bottom_right"), [x + width, y])?,
            self.add_named_point(format!("{label}.top_right"), [x + width, y + height])?,
            self.add_named_point(format!("{label}.top_left"), [x, y + height])?,
        ];
        let pairs = [(0, 1), (1, 2), (2, 3), (3, 0)];
        let curve_values = pairs
            .into_iter()
            .enumerate()
            .map(|(index, (start, end))| {
                let direction = normalized_direction(
                    self.point(points[start]).expect("new point").position,
                    self.point(points[end]).expect("new point").position,
                )?;
                self.add_curve(
                    format!("{label}.edge_{}", index + 1),
                    CurveDefinition::Line {
                        start: points[start],
                        end: points[end],
                        branch_direction: direction,
                    },
                )
            })
            .collect::<Result<Vec<_>, DocumentError>>()?;
        let curves: [CurveId; 4] = curve_values.try_into().expect("four rectangle curves");
        let constraints = [
            self.add_constraint(
                format!("{label}.anchor"),
                DocumentConstraintDefinition::FixedPoint {
                    point: points[0],
                    target: origin,
                },
            )?,
            self.add_constraint(
                format!("{label}.bottom_horizontal"),
                DocumentConstraintDefinition::Horizontal {
                    line: CurveSpan::line(curves[0]),
                },
            )?,
            self.add_constraint(
                format!("{label}.right_vertical"),
                DocumentConstraintDefinition::Vertical {
                    line: CurveSpan::line(curves[1]),
                },
            )?,
            self.add_constraint(
                format!("{label}.top_horizontal"),
                DocumentConstraintDefinition::Horizontal {
                    line: CurveSpan::line(curves[2]),
                },
            )?,
            self.add_constraint(
                format!("{label}.left_vertical"),
                DocumentConstraintDefinition::Vertical {
                    line: CurveSpan::line(curves[3]),
                },
            )?,
        ];
        let targets = [
            self.add_scalar(
                format!("{label}.width"),
                width,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )?,
            self.add_scalar(
                format!("{label}.height"),
                height,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )?,
        ];
        let dimensions = [
            self.add_dimension(
                format!("{label}.width_dimension"),
                DocumentDimensionDefinition::CurveLength {
                    curve: CurveSpan::line(curves[0]),
                    target: targets[0],
                },
                DocumentDimensionMode::Driving,
            )?,
            self.add_dimension(
                format!("{label}.height_dimension"),
                DocumentDimensionDefinition::CurveLength {
                    curve: CurveSpan::line(curves[1]),
                    target: targets[1],
                },
                DocumentDimensionMode::Driving,
            )?,
        ];
        Ok(RectangleIds {
            points,
            curves,
            anchor: constraints[0],
            constraints,
            dimensions,
            targets,
        })
    }

    /// Replaces one point position after validating the complete candidate graph.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing point or invalid candidate geometry.
    pub fn set_point_position(
        &mut self,
        id: DesignPointId,
        position: [f64; 2],
    ) -> Result<(), DocumentError> {
        finite_pair(position, "point position")?;
        let mut candidate = self.clone();
        candidate
            .points
            .iter_mut()
            .find(|value| value.id == id)
            .ok_or_else(|| unknown("point", id.0))?
            .position = position;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Replaces one scalar value without changing its identity or domain.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing scalar or a value outside its domain.
    pub fn set_scalar_value(
        &mut self,
        id: DesignScalarId,
        value: f64,
    ) -> Result<(), DocumentError> {
        if self.contacts.iter().any(|contact| contact.parameter == id) {
            return invalid(
                "scalar edit",
                "contact-owned scalars require an atomic contact-state edit",
            );
        }
        if self.curves.iter().any(|curve| {
            matches!(
                &curve.definition,
                CurveDefinition::Nurbs { gauge_weight, .. } if *gauge_weight == id
            )
        }) {
            return invalid(
                "scalar edit",
                "the selected NURBS gauge weight requires an explicit gauge transaction",
            );
        }
        let mut candidate = self.clone();
        let scalar = candidate
            .scalars
            .iter_mut()
            .find(|scalar| scalar.id == id)
            .ok_or_else(|| unknown("scalar", id.0))?;
        validate_scalar_value(value, scalar.domain)?;
        scalar.value = value;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Selects a new exact unit NURBS weight gauge without changing geometry.
    ///
    /// Every owned weight is divided by the selected weight's current value in one
    /// validated transaction. Direct edits of the selected gauge remain forbidden.
    ///
    /// # Errors
    ///
    /// Rejects a missing/non-NURBS curve, a foreign weight, or a non-finite
    /// projective rescaling.
    pub fn set_nurbs_weight_gauge(
        &mut self,
        curve: CurveId,
        gauge_weight: DesignScalarId,
    ) -> Result<(), DocumentError> {
        let mut candidate = self.clone();
        let weights = match &candidate
            .curve(curve)
            .ok_or_else(|| unknown("curve", curve.0))?
            .definition
        {
            CurveDefinition::Nurbs { weights, .. } => weights.clone(),
            _ => return invalid("curve", "weight gauge edit requires a NURBS curve"),
        };
        if !weights.contains(&gauge_weight) {
            return invalid(
                "curve.gauge_weight",
                "new NURBS gauge must select one owned weight scalar",
            );
        }
        let scale = candidate.require_scalar(gauge_weight)?.value;
        finite_positive(scale, "NURBS gauge weight")?;
        for weight in &weights {
            let scalar = candidate
                .scalar_mut(*weight)
                .ok_or_else(|| unknown("scalar", weight.0))?;
            let normalized = scalar.value / scale;
            finite_positive(normalized, "normalized NURBS weight")?;
            scalar.value = normalized;
        }
        candidate
            .scalar_mut(gauge_weight)
            .ok_or_else(|| unknown("scalar", gauge_weight.0))?
            .value = 1.0;
        let CurveDefinition::Nurbs {
            gauge_weight: selected,
            ..
        } = &mut candidate
            .curve_mut(curve)
            .ok_or_else(|| unknown("curve", curve.0))?
            .definition
        else {
            return invalid("curve", "weight gauge curve family changed");
        };
        *selected = gauge_weight;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Replaces the homogeneous weighted middle coordinate of a rational quadratic conic.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale/wrong-family curve or invalid resulting conic geometry.
    pub fn set_conic_weighted_middle(
        &mut self,
        curve: CurveId,
        weighted_middle: [f64; 2],
    ) -> Result<(), DocumentError> {
        finite_pair(weighted_middle, "conic weighted_middle")?;
        let mut candidate = self.clone();
        let value = candidate
            .curves
            .iter_mut()
            .find(|value| value.id == curve)
            .ok_or_else(|| unknown("curve", curve.0))?;
        let CurveDefinition::RationalQuadraticConic {
            weighted_middle: current,
            ..
        } = &mut value.definition
        else {
            return invalid(
                "curve",
                "weighted-middle edit requires a rational quadratic conic",
            );
        };
        *current = weighted_middle;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Explicitly changes one persistent hyperbola segment's selected branch.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale/wrong-family curve or invalid resulting conic geometry.
    pub fn set_hyperbola_branch(
        &mut self,
        curve: CurveId,
        branch: DocumentHyperbolaBranch,
    ) -> Result<(), DocumentError> {
        let mut candidate = self.clone();
        let value = candidate
            .curves
            .iter_mut()
            .find(|value| value.id == curve)
            .ok_or_else(|| unknown("curve", curve.0))?;
        let CurveDefinition::HyperbolaSegment {
            branch: current, ..
        } = &mut value.definition
        else {
            return invalid("curve", "branch edit requires a hyperbola segment");
        };
        *current = branch;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Replaces one line/polyline segment branch without changing curve identity.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing curve, invalid segment, or incompatible branch.
    pub fn set_curve_branch(
        &mut self,
        span: CurveSpan,
        direction: [f64; 2],
    ) -> Result<(), DocumentError> {
        let mut candidate = self.clone();
        let direction = normalized_vector(direction)?;
        let current = candidate.current_curve_span_direction(span)?;
        if dot(current, direction) <= 0.0 {
            return invalid(
                "curve branch",
                "selected direction must contain the current line orientation",
            );
        }
        let curve = candidate
            .curves
            .iter_mut()
            .find(|curve| curve.id == span.curve)
            .ok_or_else(|| unknown("curve", span.curve.0))?;
        match &mut curve.definition {
            CurveDefinition::Line {
                branch_direction, ..
            } if span.segment == 0 => *branch_direction = direction,
            CurveDefinition::Polyline {
                branch_directions, ..
            } => {
                let branch = branch_directions
                    .get_mut(span.segment as usize)
                    .ok_or_else(|| DocumentError::InvalidField {
                        field: "curve span",
                        message: "segment index is outside the polyline".into(),
                    })?;
                *branch = direction;
            }
            _ => return invalid("curve span", "branch edit requires a line segment"),
        }
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Explicitly selects the branch represented by a line span's current direction.
    ///
    /// This is useful when a previously unconstrained line is about to gain an axis/length root.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing curve, invalid segment, or degenerate current direction.
    pub fn reselect_curve_branch(&mut self, span: CurveSpan) -> Result<(), DocumentError> {
        let direction = self.current_curve_span_direction(span)?;
        self.set_curve_branch(span, direction)
    }

    /// Replaces a circular or elliptical arc's explicit sweep branch.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing/non-arc curve or invalid resulting sweep.
    pub fn set_arc_sweep(
        &mut self,
        curve: CurveId,
        sweep: DocumentArcSweep,
    ) -> Result<(), DocumentError> {
        let mut candidate = self.clone();
        let value = candidate
            .curves
            .iter_mut()
            .find(|value| value.id == curve)
            .ok_or_else(|| unknown("curve", curve.0))?;
        let (CurveDefinition::CircularArc { sweep: current, .. }
        | CurveDefinition::EllipticalArc { sweep: current, .. }) = &mut value.definition
        else {
            return invalid("curve", "sweep edit requires a circular or elliptical arc");
        };
        *current = sweep;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Atomically replaces one point contact or both contacts of one tangency source.
    ///
    /// # Errors
    ///
    /// Returns an error for missing/repeated contacts, invalid values, or inconsistent state.
    pub fn set_contact_states(&mut self, edits: &[ContactStateEdit]) -> Result<(), DocumentError> {
        if edits.is_empty() || edits.len() > 2 {
            return invalid(
                "contact edits",
                "must contain one point contact or two tangency contacts",
            );
        }
        let requested = edits
            .iter()
            .map(|edit| edit.contact)
            .collect::<BTreeSet<_>>();
        if requested.len() != edits.len() {
            return invalid("contact edits", "contact IDs must be distinct");
        }
        self.ordered_source_contacts(&requested.iter().copied().collect::<Vec<_>>())?;
        let mut candidate = self.clone();
        for edit in edits {
            let (parameter, domain) = {
                let contact = candidate
                    .contacts
                    .iter_mut()
                    .find(|contact| contact.id == edit.contact)
                    .ok_or_else(|| unknown("contact", edit.contact.0))?;
                contact.winding = edit.winding;
                contact.neighborhood = edit.neighborhood;
                contact.tangent_orientation = edit.tangent_orientation;
                (contact.parameter, contact.domain)
            };
            if let ContactDomain::Bounded { lower, upper } = domain
                && !(lower..=upper).contains(&edit.value)
            {
                return Err(DocumentError::ContactParameterOutOfDomain {
                    contact: edit.contact,
                    value: edit.value,
                    lower,
                    upper,
                });
            }
            let scalar = candidate
                .scalars
                .iter_mut()
                .find(|scalar| scalar.id == parameter)
                .ok_or_else(|| unknown("scalar", parameter.0))?;
            validate_scalar_value(edit.value, scalar.domain)?;
            scalar.value = edit.value;
        }
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Returns selected contacts in the owning source's semantic order.
    ///
    /// # Errors
    ///
    /// Returns an error unless the distinct IDs exactly cover one contact-owning source.
    pub fn ordered_source_contacts(
        &self,
        contacts: &[ContactId],
    ) -> Result<Vec<ContactId>, DocumentError> {
        if contacts.is_empty() || contacts.len() > 2 {
            return invalid(
                "contact edits",
                "must contain one point contact or two tangency contacts",
            );
        }
        let requested = contacts.iter().copied().collect::<BTreeSet<_>>();
        if requested.len() != contacts.len() {
            return invalid("contact edits", "contact IDs must be distinct");
        }
        let mut owning_sources = self.constraints.iter().filter_map(|constraint| {
            let contacts = constraint_contacts(&constraint.definition);
            contacts
                .iter()
                .any(|contact| requested.contains(contact))
                .then_some(contacts)
        });
        let Some(owned) = owning_sources.next() else {
            return invalid("contact edits", "contacts must belong to one source");
        };
        if owning_sources.next().is_some()
            || owned.len() != requested.len()
            || !owned.iter().all(|contact| requested.contains(contact))
        {
            return invalid(
                "contact edits",
                "all contacts of exactly one source must be edited atomically",
            );
        }
        Ok(owned)
    }

    /// Replaces circle-circle tangency mode and center-direction branch in place.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing/wrong source or invalid branch state.
    pub fn set_circle_tangency_branch(
        &mut self,
        id: DocumentConstraintId,
        mode: DocumentCircleTangencyMode,
        center_direction: [f64; 2],
    ) -> Result<(), DocumentError> {
        let mut candidate = self.clone();
        let constraint = candidate
            .constraints
            .iter_mut()
            .find(|constraint| constraint.id == id)
            .ok_or_else(|| unknown("constraint", id.0))?;
        let DocumentConstraintDefinition::CircleCircleTangency {
            mode: current_mode,
            center_direction: current_direction,
            ..
        } = &mut constraint.definition
        else {
            return invalid("constraint", "branch edit requires circle-circle tangency");
        };
        *current_mode = mode;
        *current_direction = normalized_vector(center_direction)?;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Changes driving/reference state while retaining source identity and order.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing dimension.
    pub fn set_dimension_mode(
        &mut self,
        id: DocumentDimensionId,
        mode: DocumentDimensionMode,
    ) -> Result<(), DocumentError> {
        self.dimensions
            .iter_mut()
            .find(|value| value.id == id)
            .ok_or_else(|| unknown("dimension", id.0))?
            .mode = mode;
        Ok(())
    }

    /// Suppresses or unsuppresses one persistent source without deleting it.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing source.
    pub fn set_source_suppressed(
        &mut self,
        source: DocumentSourceId,
        suppressed: bool,
    ) -> Result<(), DocumentError> {
        if let Some(constraint) = self
            .constraints
            .iter_mut()
            .find(|value| value.source_id == source)
        {
            constraint.suppressed = suppressed;
            return Ok(());
        }
        if let Some(dimension) = self
            .dimensions
            .iter_mut()
            .find(|value| value.source_id == source)
        {
            dimension.suppressed = suppressed;
            return Ok(());
        }
        Err(unknown("source", source.0))
    }

    /// Replaces one persistent source's human-readable audit label.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid label or unknown source identity.
    pub fn set_source_label(
        &mut self,
        source: DocumentSourceId,
        label: impl Into<String>,
    ) -> Result<(), DocumentError> {
        let label = label.into();
        validate_label(&label, "source label")?;
        if let Some(constraint) = self
            .constraints
            .iter_mut()
            .find(|value| value.source_id == source)
        {
            constraint.label = label;
            return Ok(());
        }
        if let Some(dimension) = self
            .dimensions
            .iter_mut()
            .find(|value| value.source_id == source)
        {
            dimension.label = label;
            return Ok(());
        }
        Err(unknown("source", source.0))
    }

    /// Deletes one unreferenced object. IDs are never recycled.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing or still-referenced object.
    pub fn remove(&mut self, object: DocumentObjectId) -> Result<(), DocumentError> {
        let mut candidate = self.clone();
        match object {
            DocumentObjectId::Point(id) => retain_remove(&mut candidate.points, |v| v.id == id)
                .then_some(())
                .ok_or_else(|| unknown("point", id.0))?,
            DocumentObjectId::Scalar(id) => retain_remove(&mut candidate.scalars, |v| v.id == id)
                .then_some(())
                .ok_or_else(|| unknown("scalar", id.0))?,
            DocumentObjectId::Curve(id) => {
                let owned_scalars = curve_owned_scalars(
                    &candidate
                        .curve(id)
                        .ok_or_else(|| unknown("curve", id.0))?
                        .definition,
                );
                retain_remove(&mut candidate.curves, |value| value.id == id);
                candidate
                    .scalars
                    .retain(|value| !owned_scalars.contains(&value.id));
            }
            DocumentObjectId::Contact(id) => retain_remove(&mut candidate.contacts, |v| v.id == id)
                .then_some(())
                .ok_or_else(|| unknown("contact", id.0))?,
            DocumentObjectId::Constraint(id) => {
                let source = candidate
                    .constraint(id)
                    .ok_or_else(|| unknown("constraint", id.0))?
                    .source_id;
                retain_remove(&mut candidate.constraints, |value| value.id == id);
                candidate.source_order.retain(|value| *value != source);
            }
            DocumentObjectId::Dimension(id) => {
                let source = candidate
                    .dimension(id)
                    .ok_or_else(|| unknown("dimension", id.0))?
                    .source_id;
                retain_remove(&mut candidate.dimensions, |value| value.id == id);
                candidate.source_order.retain(|value| *value != source);
            }
        }
        candidate.validate().map_err(|error| match error {
            DocumentError::UnknownId { .. } => {
                DocumentError::ObjectInUse(object_persistent(object))
            }
            other => other,
        })?;
        *self = candidate;
        Ok(())
    }

    /// Deletes one object and the private contact/target state owned by a deleted source.
    ///
    /// Geometry deletion remains conservative and rejects while referenced. Deleting a
    /// constraint additionally removes its now-unreferenced contact slots and parameter scalars;
    /// deleting a dimension removes its now-unreferenced target scalar.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing object or geometry that remains referenced.
    pub fn remove_with_owned_state(
        &mut self,
        object: DocumentObjectId,
    ) -> Result<(), DocumentError> {
        let mut candidate = self.clone();
        match object {
            DocumentObjectId::Constraint(id) => {
                let contacts = constraint_contacts(
                    &candidate
                        .constraint(id)
                        .ok_or_else(|| unknown("constraint", id.0))?
                        .definition,
                );
                let parameters: Vec<_> = contacts
                    .iter()
                    .filter_map(|contact| candidate.contact(*contact).map(|slot| slot.parameter))
                    .collect();
                candidate.remove(object)?;
                for contact in contacts {
                    let _ = candidate.remove(DocumentObjectId::Contact(contact));
                }
                for parameter in parameters {
                    let _ = candidate.remove(DocumentObjectId::Scalar(parameter));
                }
            }
            DocumentObjectId::Dimension(id) => {
                let target = dimension_target(
                    &candidate
                        .dimension(id)
                        .ok_or_else(|| unknown("dimension", id.0))?
                        .definition,
                );
                candidate.remove(object)?;
                let _ = candidate.remove(DocumentObjectId::Scalar(target));
            }
            DocumentObjectId::Curve(id) => {
                let owned_scalars = curve_owned_scalars(
                    &candidate
                        .curve(id)
                        .ok_or_else(|| unknown("curve", id.0))?
                        .definition,
                );
                candidate.remove(object)?;
                for scalar in owned_scalars {
                    let _ = candidate.remove(DocumentObjectId::Scalar(scalar));
                }
            }
            DocumentObjectId::Contact(id) => {
                let parameter = candidate
                    .contact(id)
                    .ok_or_else(|| unknown("contact", id.0))?
                    .parameter;
                candidate.remove(object)?;
                let _ = candidate.remove(DocumentObjectId::Scalar(parameter));
            }
            _ => candidate.remove(object)?,
        }
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Deletes selected objects, every object that depends on them, and private owned state.
    ///
    /// A selected point therefore removes curves and sources that reference it, while selecting a
    /// curve alone retains its control points. The complete deletion is validated atomically.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing selected object or an invalid resulting document.
    pub fn remove_many_with_dependents(
        &mut self,
        objects: &[DocumentObjectId],
    ) -> Result<(), DocumentError> {
        let mut removal = objects.iter().copied().collect::<BTreeSet<_>>();
        for object in &removal {
            if !self.contains_object(*object) {
                return Err(unknown("object", object_persistent(*object)));
            }
        }
        loop {
            let previous_len = removal.len();
            for curve in &self.curves {
                if removal
                    .iter()
                    .any(|object| curve_references_object(&curve.definition, *object))
                {
                    removal.insert(DocumentObjectId::Curve(curve.id));
                }
            }
            for contact in &self.contacts {
                if removal
                    .iter()
                    .any(|object| contact_references_object(contact, *object))
                {
                    removal.insert(DocumentObjectId::Contact(contact.id));
                }
            }
            for constraint in &self.constraints {
                if removal
                    .iter()
                    .any(|object| constraint_references_object(&constraint.definition, *object))
                {
                    removal.insert(DocumentObjectId::Constraint(constraint.id));
                }
            }
            for dimension in &self.dimensions {
                if removal
                    .iter()
                    .any(|object| dimension_references_object(&dimension.definition, *object))
                {
                    removal.insert(DocumentObjectId::Dimension(dimension.id));
                }
            }
            if removal.len() == previous_len {
                break;
            }
        }

        let mut removal = removal.into_iter().collect::<Vec<_>>();
        removal.sort_by_key(|object| match object {
            DocumentObjectId::Constraint(_) | DocumentObjectId::Dimension(_) => 0,
            DocumentObjectId::Contact(_) => 1,
            DocumentObjectId::Curve(_) => 2,
            DocumentObjectId::Point(_) | DocumentObjectId::Scalar(_) => 3,
        });
        let mut candidate = self.clone();
        for object in removal {
            if let Err(error) = candidate.remove_with_owned_state(object)
                && !matches!(error, DocumentError::UnknownId { .. })
            {
                return Err(error);
            }
        }
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Validates schema, resources, finite values, references, domains, and source order.
    ///
    /// # Errors
    ///
    /// Returns the first schema, resource, value, reference, or ordering error.
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Result<(), DocumentError> {
        if self.version != SKETCH_DOCUMENT_VERSION {
            return Err(DocumentError::UnsupportedVersion {
                actual: self.version,
                expected: SKETCH_DOCUMENT_VERSION,
            });
        }
        finite_positive(self.model_scale, "model_scale")?;
        let curve_point_references: usize = self
            .curves
            .iter()
            .map(|curve| match &curve.definition {
                CurveDefinition::Polyline { points, .. } => points.len(),
                CurveDefinition::QuadraticBezier { controls } => controls.len(),
                CurveDefinition::CubicBezier { controls } => controls.len(),
                CurveDefinition::BSpline { controls, .. }
                | CurveDefinition::Nurbs { controls, .. } => controls.len(),
                CurveDefinition::Ellipse { .. }
                | CurveDefinition::EllipticalArc { .. }
                | CurveDefinition::RationalQuadraticConic { .. }
                | CurveDefinition::ParabolaSegment { .. }
                | CurveDefinition::HyperbolaSegment { .. } => 2,
                _ => 0,
            })
            .sum();
        let count = 1
            + self.points.len()
            + self.scalars.len()
            + self.curves.len()
            + self.contacts.len()
            + self.constraints.len() * 2
            + self.dimensions.len() * 2
            + curve_point_references;
        if count > MAX_DOCUMENT_OBJECTS {
            return Err(DocumentError::ResourceLimit {
                resource: "objects",
                actual: count,
                limit: MAX_DOCUMENT_OBJECTS,
            });
        }
        let mut ids = BTreeSet::new();
        insert_unique(&mut ids, self.id.0)?;
        for point in &self.points {
            insert_unique(&mut ids, point.id.0)?;
            validate_label(&point.label, "point label")?;
            finite_pair(point.position, "point position")?;
        }
        for scalar in &self.scalars {
            insert_unique(&mut ids, scalar.id.0)?;
            validate_label(&scalar.label, "scalar label")?;
            validate_scalar_value(scalar.value, scalar.domain)?;
        }
        let mut used_scalars = BTreeSet::new();
        for curve in &self.curves {
            insert_unique(&mut ids, curve.id.0)?;
            validate_label(&curve.label, "curve label")?;
            self.validate_curve_definition(curve.id, &curve.definition)?;
            for scalar in curve_scalars(&curve.definition) {
                claim_scalar(&mut used_scalars, scalar)?;
            }
        }
        let mut contact_scalars = BTreeSet::new();
        for contact in &self.contacts {
            insert_unique(&mut ids, contact.id.0)?;
            validate_label(&contact.label, "contact label")?;
            let curve = self.validate_span(contact.curve)?;
            let scalar = self
                .scalar(contact.parameter)
                .ok_or_else(|| unknown("scalar", contact.parameter.0))?;
            validate_contact(
                contact,
                scalar,
                matches!(
                    curve.definition,
                    CurveDefinition::BSpline {
                        form: DocumentBSplineForm::Periodic,
                        ..
                    } | CurveDefinition::Nurbs {
                        form: DocumentBSplineForm::Periodic,
                        ..
                    }
                ),
            )?;
            validate_contact_curve(contact, scalar, curve)?;
            claim_scalar(&mut used_scalars, contact.parameter)?;
            if !contact_scalars.insert(contact.parameter) {
                return invalid(
                    "contact.parameter",
                    "each contact slot must own a distinct scalar identity",
                );
            }
        }
        let mut sources = BTreeSet::new();
        let mut used_contacts = BTreeSet::new();
        for constraint in &self.constraints {
            insert_unique(&mut ids, constraint.id.0)?;
            insert_unique(&mut ids, constraint.source_id.0)?;
            sources.insert(constraint.source_id);
            validate_label(&constraint.label, "constraint label")?;
            self.validate_constraint_definition(&constraint.definition)?;
            for contact in constraint_contacts(&constraint.definition) {
                if !used_contacts.insert(contact) {
                    return invalid(
                        "constraint contact",
                        "a contact slot may belong to only one constraint source",
                    );
                }
            }
        }
        for dimension in &self.dimensions {
            insert_unique(&mut ids, dimension.id.0)?;
            insert_unique(&mut ids, dimension.source_id.0)?;
            sources.insert(dimension.source_id);
            validate_label(&dimension.label, "dimension label")?;
            self.validate_dimension_definition(&dimension.definition)?;
            claim_scalar(&mut used_scalars, dimension_target(&dimension.definition))?;
        }
        let ordered: BTreeSet<_> = self.source_order.iter().copied().collect();
        if ordered.len() != self.source_order.len() || ordered != sources {
            return invalid("source_order", "must contain every source exactly once");
        }
        let maximum = ids.iter().map(|id| id.as_u128()).max().unwrap_or(0);
        if self.next_id.as_u128() == 0 || self.next_id.as_u128() <= maximum {
            return invalid(
                "next_id",
                "must be greater than every allocated persistent ID",
            );
        }
        Ok(())
    }

    /// Serializes a normalized deterministic JSON representation.
    ///
    /// # Errors
    ///
    /// Returns a validation or JSON serialization error.
    pub fn to_canonical_json(&self) -> Result<String, DocumentError> {
        self.validate()?;
        let mut canonical = self.clone();
        canonical.points.sort_by_key(|value| value.id);
        canonical.scalars.sort_by_key(|value| value.id);
        canonical.curves.sort_by_key(|value| value.id);
        canonical.contacts.sort_by_key(|value| value.id);
        canonical.constraints.sort_by_key(|value| value.id);
        canonical.dimensions.sort_by_key(|value| value.id);
        Ok(serde_json::to_string(&canonical)?)
    }

    /// Parses and strictly validates a versioned JSON document.
    ///
    /// # Errors
    ///
    /// Returns a JSON, schema, resource, value, reference, or ordering error.
    pub fn from_json(json: &str) -> Result<Self, DocumentError> {
        if json.len() > MAX_DOCUMENT_JSON_BYTES {
            return Err(DocumentError::ResourceLimit {
                resource: "JSON bytes",
                actual: json.len(),
                limit: MAX_DOCUMENT_JSON_BYTES,
            });
        }
        let header: DocumentHeader = serde_json::from_str(json)?;
        if header.version != SKETCH_DOCUMENT_VERSION {
            return Err(DocumentError::UnsupportedVersion {
                actual: header.version,
                expected: SKETCH_DOCUMENT_VERSION,
            });
        }
        let mut document: Self = serde_json::from_str(json)?;
        document.validate()?;
        document.points.sort_by_key(|value| value.id);
        document.scalars.sort_by_key(|value| value.id);
        document.curves.sort_by_key(|value| value.id);
        document.contacts.sort_by_key(|value| value.id);
        document.constraints.sort_by_key(|value| value.id);
        document.dimensions.sort_by_key(|value| value.id);
        Ok(document)
    }

    pub(crate) fn point_mut(&mut self, id: DesignPointId) -> Option<&mut DesignPoint> {
        self.points.iter_mut().find(|value| value.id == id)
    }

    pub(crate) fn scalar_mut(&mut self, id: DesignScalarId) -> Option<&mut DesignScalar> {
        self.scalars.iter_mut().find(|value| value.id == id)
    }

    pub(crate) fn curve_mut(&mut self, id: CurveId) -> Option<&mut DesignCurve> {
        self.curves.iter_mut().find(|value| value.id == id)
    }

    pub(crate) fn contact_mut(&mut self, id: ContactId) -> Option<&mut ContactSlot> {
        self.contacts.iter_mut().find(|value| value.id == id)
    }

    pub(crate) const fn allocator_cursor(&self) -> PersistentId {
        self.next_id
    }

    pub(crate) fn spline_span_allocator_cursors(&self) -> BTreeMap<CurveId, u32> {
        self.curves
            .iter()
            .filter_map(|curve| {
                let (CurveDefinition::BSpline { next_span_id, .. }
                | CurveDefinition::Nurbs { next_span_id, .. }) = &curve.definition
                else {
                    return None;
                };
                Some((curve.id, *next_span_id))
            })
            .collect()
    }

    pub(crate) fn advance_allocator(&mut self, cursor: PersistentId) {
        if cursor > self.next_id {
            self.next_id = cursor;
        }
    }

    pub(crate) fn advance_spline_span_allocators(&mut self, cursors: &BTreeMap<CurveId, u32>) {
        for curve in &mut self.curves {
            let (CurveDefinition::BSpline { next_span_id, .. }
            | CurveDefinition::Nurbs { next_span_id, .. }) = &mut curve.definition
            else {
                continue;
            };
            if let Some(retained) = cursors.get(&curve.id) {
                *next_span_id = (*next_span_id).max(*retained);
            }
        }
    }

    pub(crate) fn curve_branch_is_enforced(&self, span: CurveSpan) -> bool {
        let has_axis_constraint = self
            .constraints
            .iter()
            .filter(|constraint| !constraint.suppressed)
            .any(|constraint| {
                matches!(
                    constraint.definition,
                    DocumentConstraintDefinition::Horizontal { line }
                        | DocumentConstraintDefinition::Vertical { line }
                        if line == span
                )
            });
        let has_driving_length = self
            .dimensions
            .iter()
            .filter(|dimension| {
                !dimension.suppressed && dimension.mode == DocumentDimensionMode::Driving
            })
            .any(|dimension| {
                matches!(
                    dimension.definition,
                    DocumentDimensionDefinition::CurveLength { curve, .. } if curve == span
                )
            });
        has_axis_constraint && has_driving_length
    }

    pub(crate) fn current_curve_span_direction(
        &self,
        span: CurveSpan,
    ) -> Result<[f64; 2], DocumentError> {
        let curve = self
            .curve(span.curve)
            .ok_or_else(|| unknown("curve", span.curve.0))?;
        let (start, end) = match &curve.definition {
            CurveDefinition::Line { start, end, .. } if span.segment == 0 => (*start, *end),
            CurveDefinition::Polyline { points, closed, .. } => {
                let start =
                    usize::try_from(span.segment).map_err(|_| DocumentError::InvalidField {
                        field: "curve span",
                        message: "segment index is outside the polyline".into(),
                    })?;
                let end = if start + 1 == points.len() && *closed {
                    0
                } else {
                    start + 1
                };
                (
                    *points
                        .get(start)
                        .ok_or_else(|| DocumentError::InvalidField {
                            field: "curve span",
                            message: "segment index is outside the polyline".into(),
                        })?,
                    *points.get(end).ok_or_else(|| DocumentError::InvalidField {
                        field: "curve span",
                        message: "segment index is outside the polyline".into(),
                    })?,
                )
            }
            _ => return invalid("curve span", "branch direction requires a line segment"),
        };
        normalized_direction(
            self.require_point(start)?.position,
            self.require_point(end)?.position,
        )
    }

    fn contains_object(&self, object: DocumentObjectId) -> bool {
        match object {
            DocumentObjectId::Point(id) => self.point(id).is_some(),
            DocumentObjectId::Scalar(id) => self.scalar(id).is_some(),
            DocumentObjectId::Curve(id) => self.curve(id).is_some(),
            DocumentObjectId::Contact(id) => self.contact(id).is_some(),
            DocumentObjectId::Constraint(id) => self.constraint(id).is_some(),
            DocumentObjectId::Dimension(id) => self.dimension(id).is_some(),
        }
    }

    fn allocate_id(&mut self) -> Result<PersistentId, DocumentError> {
        let id = self.next_id;
        self.next_id = PersistentId(
            self.next_id
                .as_u128()
                .checked_add(1)
                .ok_or(DocumentError::IdExhausted)?,
        );
        Ok(id)
    }

    #[allow(clippy::too_many_lines)]
    fn validate_curve_definition(
        &self,
        curve: CurveId,
        definition: &CurveDefinition,
    ) -> Result<(), DocumentError> {
        match definition {
            CurveDefinition::Line {
                start,
                end,
                branch_direction,
            } => {
                self.require_point(*start)?;
                self.require_point(*end)?;
                if start == end {
                    return invalid("curve.definition", "line endpoints must be distinct");
                }
                validate_unit_direction(*branch_direction, "line branch_direction")?;
                let current = normalized_direction(
                    self.require_point(*start)?.position,
                    self.require_point(*end)?.position,
                )?;
                if self.curve_branch_is_enforced(CurveSpan::line(curve))
                    && dot(current, *branch_direction) <= 0.0
                {
                    return invalid(
                        "curve.branch_direction",
                        "current line is on the opposite branch",
                    );
                }
            }
            CurveDefinition::Polyline {
                points,
                closed,
                branch_directions,
            } => {
                if points.len() > MAX_POLYLINE_POINTS {
                    return Err(DocumentError::ResourceLimit {
                        resource: "polyline points",
                        actual: points.len(),
                        limit: MAX_POLYLINE_POINTS,
                    });
                }
                if points.len() < 2 {
                    return invalid("curve.definition", "polyline requires at least two points");
                }
                let segments = points.len() - 1 + usize::from(*closed);
                if branch_directions.len() != segments {
                    return invalid("curve.branch_directions", "must match the segment count");
                }
                for (index, direction) in branch_directions.iter().enumerate() {
                    let start = self.require_point(points[index])?;
                    let end_index = if index + 1 == points.len() {
                        0
                    } else {
                        index + 1
                    };
                    let end = self.require_point(points[end_index])?;
                    if start.id == end.id {
                        return invalid(
                            "curve.definition",
                            "polyline segment endpoints must differ",
                        );
                    }
                    validate_unit_direction(*direction, "polyline branch_direction")?;
                    if self.curve_branch_is_enforced(CurveSpan {
                        curve,
                        segment: u32::try_from(index).map_err(|_| {
                            DocumentError::ResourceLimit {
                                resource: "polyline segment index",
                                actual: index,
                                limit: u32::MAX as usize,
                            }
                        })?,
                    }) && dot(
                        normalized_direction(start.position, end.position)?,
                        *direction,
                    ) <= 0.0
                    {
                        return invalid(
                            "curve.branch_directions",
                            "current polyline segment is on the opposite branch",
                        );
                    }
                }
            }
            CurveDefinition::Circle { center, radius } => {
                self.require_point(*center)?;
                let radius = self.require_scalar(*radius)?;
                finite_positive(radius.value, "circle radius")?;
                require_scalar_role(
                    radius,
                    ScalarUnit::Length,
                    ScalarDomain::Positive,
                    "circle radius",
                )?;
            }
            CurveDefinition::CircularArc {
                center,
                radius,
                start_angle,
                end_angle,
                ..
            } => {
                self.require_point(*center)?;
                let radius = self.require_scalar(*radius)?;
                finite_positive(radius.value, "arc radius")?;
                require_scalar_role(
                    radius,
                    ScalarUnit::Length,
                    ScalarDomain::Positive,
                    "arc radius",
                )?;
                let start = self.require_scalar(*start_angle)?;
                let end = self.require_scalar(*end_angle)?;
                require_scalar_role(
                    start,
                    ScalarUnit::Angle,
                    ScalarDomain::Finite,
                    "arc start angle",
                )?;
                require_scalar_role(
                    end,
                    ScalarUnit::Angle,
                    ScalarDomain::Finite,
                    "arc end angle",
                )?;
                finite(start.value, "arc start_angle")?;
                finite(end.value, "arc end_angle")?;
                let difference = (end.value - start.value).rem_euclid(std::f64::consts::TAU);
                if !difference.is_finite() || difference == 0.0 {
                    return invalid("curve.definition", "arc sweep must be nonzero");
                }
            }
            CurveDefinition::QuadraticBezier { controls } => {
                for control in controls {
                    self.require_point(*control)?;
                }
            }
            CurveDefinition::CubicBezier { controls } => {
                for control in controls {
                    self.require_point(*control)?;
                }
            }
            CurveDefinition::BSpline {
                form,
                degree,
                controls,
                knots,
                span_ids,
                next_span_id,
            } => {
                if controls.len() > MAX_BSPLINE_CONTROLS {
                    return Err(DocumentError::ResourceLimit {
                        resource: "B-spline controls",
                        actual: controls.len(),
                        limit: MAX_BSPLINE_CONTROLS,
                    });
                }
                let mut unique_controls = BTreeSet::new();
                for control in controls {
                    self.require_point(*control)?;
                    if !unique_controls.insert(*control) {
                        return invalid(
                            "curve.controls",
                            "B-spline control identities must be distinct",
                        );
                    }
                }
                let basis = match form {
                    DocumentBSplineForm::Clamped => geosolve_geometry::BSplineBasis::try_clamped(
                        *degree,
                        controls.len(),
                        knots.clone(),
                    ),
                    DocumentBSplineForm::Periodic => geosolve_geometry::BSplineBasis::try_periodic(
                        *degree,
                        controls.len(),
                        knots.clone(),
                    ),
                }
                .map_err(|source| DocumentError::BSplineDefinition { curve, source })?;
                if span_ids.len() != basis.spans().len() {
                    return invalid(
                        "curve.span_ids",
                        "must contain one semantic ID per positive knot span",
                    );
                }
                let unique_spans = span_ids.iter().copied().collect::<BTreeSet<_>>();
                if unique_spans.len() != span_ids.len() {
                    return invalid("curve.span_ids", "semantic span IDs must be unique");
                }
                let maximum = span_ids.iter().copied().max().unwrap_or(0);
                if *next_span_id <= maximum {
                    return invalid(
                        "curve.next_span_id",
                        "must be greater than every allocated semantic span ID",
                    );
                }
            }
            CurveDefinition::Nurbs {
                form,
                degree,
                controls,
                weights,
                gauge_weight,
                knots,
                span_ids,
                next_span_id,
            } => {
                if controls.len() > MAX_BSPLINE_CONTROLS {
                    return Err(DocumentError::ResourceLimit {
                        resource: "NURBS controls",
                        actual: controls.len(),
                        limit: MAX_BSPLINE_CONTROLS,
                    });
                }
                if weights.len() != controls.len() {
                    return invalid(
                        "curve.weights",
                        "NURBS requires one weight scalar per control",
                    );
                }
                let mut unique_controls = BTreeSet::new();
                let mut control_positions = Vec::with_capacity(controls.len());
                for control in controls {
                    let point = self.require_point(*control)?;
                    if !unique_controls.insert(*control) {
                        return invalid(
                            "curve.controls",
                            "NURBS control identities must be distinct",
                        );
                    }
                    control_positions.push(geosolve_geometry::Point2::new(
                        point.position[0],
                        point.position[1],
                    ));
                }
                let mut unique_weights = BTreeSet::new();
                let mut weight_values = Vec::with_capacity(weights.len());
                for weight in weights {
                    let scalar = self.require_scalar(*weight)?;
                    if !unique_weights.insert(*weight) {
                        return invalid(
                            "curve.weights",
                            "NURBS weight identities must be distinct",
                        );
                    }
                    require_scalar_role(
                        scalar,
                        ScalarUnit::Parameter,
                        ScalarDomain::Positive,
                        "NURBS weight",
                    )?;
                    finite_positive(scalar.value, "NURBS weight")?;
                    weight_values.push(scalar.value);
                }
                if !unique_weights.contains(gauge_weight) {
                    return invalid(
                        "curve.gauge_weight",
                        "NURBS gauge must select one owned weight scalar",
                    );
                }
                let gauge = self.require_scalar(*gauge_weight)?;
                if gauge.value.to_bits() != 1.0f64.to_bits() {
                    return invalid(
                        "curve.gauge_weight",
                        "selected NURBS gauge weight must be exactly one",
                    );
                }
                let basis = match form {
                    DocumentBSplineForm::Clamped => geosolve_geometry::BSplineBasis::try_clamped(
                        *degree,
                        controls.len(),
                        knots.clone(),
                    ),
                    DocumentBSplineForm::Periodic => geosolve_geometry::BSplineBasis::try_periodic(
                        *degree,
                        controls.len(),
                        knots.clone(),
                    ),
                }
                .map_err(|source| DocumentError::BSplineDefinition { curve, source })?;
                geosolve_geometry::NurbsCurve2::try_new(
                    basis.clone(),
                    control_positions,
                    weight_values,
                )
                .map_err(|source| DocumentError::NurbsDefinition { curve, source })?;
                if span_ids.len() != basis.spans().len() {
                    return invalid(
                        "curve.span_ids",
                        "must contain one semantic ID per positive knot span",
                    );
                }
                let unique_spans = span_ids.iter().copied().collect::<BTreeSet<_>>();
                if unique_spans.len() != span_ids.len() {
                    return invalid("curve.span_ids", "semantic span IDs must be unique");
                }
                let maximum = span_ids.iter().copied().max().unwrap_or(0);
                if *next_span_id <= maximum {
                    return invalid(
                        "curve.next_span_id",
                        "must be greater than every allocated semantic span ID",
                    );
                }
            }
            CurveDefinition::Ellipse {
                center,
                major_axis_point,
                minor_axis_ratio,
            } => {
                self.require_distinct_noncoincident_points(*center, *major_axis_point)?;
                require_scalar_role(
                    self.require_scalar(*minor_axis_ratio)?,
                    ScalarUnit::Parameter,
                    conic_ratio_domain(),
                    "ellipse minor-axis ratio",
                )?;
            }
            CurveDefinition::EllipticalArc {
                center,
                major_axis_point,
                minor_axis_ratio,
                start_angle,
                end_angle,
                sweep,
            } => {
                self.require_distinct_noncoincident_points(*center, *major_axis_point)?;
                require_scalar_role(
                    self.require_scalar(*minor_axis_ratio)?,
                    ScalarUnit::Parameter,
                    conic_ratio_domain(),
                    "elliptical-arc minor-axis ratio",
                )?;
                let start = self.require_scalar(*start_angle)?;
                let end = self.require_scalar(*end_angle)?;
                require_scalar_role(
                    start,
                    ScalarUnit::Angle,
                    ScalarDomain::Finite,
                    "elliptical-arc start angle",
                )?;
                require_scalar_role(
                    end,
                    ScalarUnit::Angle,
                    ScalarDomain::Finite,
                    "elliptical-arc end angle",
                )?;
                document_arc_signed_sweep(start.value, end.value, *sweep)?;
            }
            CurveDefinition::RationalQuadraticConic {
                start,
                weighted_middle,
                middle_weight,
                end,
            } => {
                self.require_distinct_noncoincident_points(*start, *end)?;
                finite_pair(*weighted_middle, "rational homogeneous middle")?;
                require_scalar_role(
                    self.require_scalar(*middle_weight)?,
                    ScalarUnit::Parameter,
                    conic_weight_domain(),
                    "rational middle weight",
                )?;
            }
            CurveDefinition::ParabolaSegment {
                vertex,
                focus,
                trim_start,
                trim_end,
            } => {
                self.require_distinct_noncoincident_points(*vertex, *focus)?;
                require_trim_scalar(self.require_scalar(*trim_start)?, "parabola trim start")?;
                require_trim_scalar(self.require_scalar(*trim_end)?, "parabola trim end")?;
                geosolve_geometry::DirectedParameterTrim::try_new(
                    self.require_scalar(*trim_start)?.value,
                    self.require_scalar(*trim_end)?.value,
                )
                .map_err(|source| DocumentError::ConicDefinition { curve, source })?;
            }
            CurveDefinition::HyperbolaSegment {
                center,
                transverse_axis_point,
                semi_conjugate,
                trim_start,
                trim_end,
                ..
            } => {
                self.require_distinct_noncoincident_points(*center, *transverse_axis_point)?;
                require_scalar_role(
                    self.require_scalar(*semi_conjugate)?,
                    ScalarUnit::Length,
                    ScalarDomain::Positive,
                    "hyperbola semi-conjugate",
                )?;
                require_trim_scalar(self.require_scalar(*trim_start)?, "hyperbola trim start")?;
                require_trim_scalar(self.require_scalar(*trim_end)?, "hyperbola trim end")?;
                geosolve_geometry::DirectedParameterTrim::try_new(
                    self.require_scalar(*trim_start)?.value,
                    self.require_scalar(*trim_end)?.value,
                )
                .map_err(|source| DocumentError::ConicDefinition { curve, source })?;
            }
        }
        if is_conic_definition(definition) {
            self.validate_conic_definition_geometry(curve, definition)?;
        }
        Ok(())
    }

    fn validate_conic_definition_geometry(
        &self,
        curve: CurveId,
        definition: &CurveDefinition,
    ) -> Result<(), DocumentError> {
        let geometry = self
            .conic_geometry(definition)
            .map_err(|error| document_conic_geometry_document_error(curve, error))?;
        let samples: &[f64] = if matches!(definition, CurveDefinition::Ellipse { .. }) {
            &[0.0, std::f64::consts::FRAC_PI_2, std::f64::consts::PI]
        } else {
            &[0.0, 0.5, 1.0]
        };
        for &parameter in samples {
            geometry
                .evaluate(parameter)
                .map_err(|source| DocumentError::ConicEvaluation { curve, source })?;
        }
        geometry
            .endpoints()
            .map_err(|source| DocumentError::ConicEvaluation { curve, source })?;
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn validate_constraint_definition(
        &self,
        definition: &DocumentConstraintDefinition,
    ) -> Result<(), DocumentError> {
        use DocumentConstraintDefinition as C;
        match definition {
            C::FixedPoint { point, target } => {
                self.require_point(*point)?;
                finite_pair(*target, "fixed-point target")?;
            }
            C::FixedCoordinate { point, target, .. } => {
                self.require_point(*point)?;
                finite(*target, "fixed-coordinate target")?;
            }
            C::Coincident { first, second } => self.require_distinct_points(*first, *second)?,
            C::Horizontal { line } | C::Vertical { line } => {
                self.validate_line_span(*line)?;
            }
            C::PointOnCurve { point, contact } => {
                self.require_point(*point)?;
                if self
                    .require_contact(*contact)?
                    .tangent_orientation
                    .is_some()
                {
                    return invalid(
                        "constraint contact",
                        "point-on-curve contact must not select tangent orientation",
                    );
                }
            }
            C::Parallel { first, second }
            | C::Perpendicular { first, second }
            | C::EqualLength { first, second } => {
                self.validate_line_span(*first)?;
                self.validate_line_span(*second)?;
                if first == second {
                    return invalid("constraint.definition", "line spans must be distinct");
                }
            }
            C::EqualRadius { first, second } => {
                self.require_circle(*first)?;
                self.require_circle(*second)?;
                if first == second {
                    return invalid("constraint.definition", "circles must be distinct");
                }
            }
            C::Midpoint { point, line } => {
                self.require_point(*point)?;
                self.validate_line_span(*line)?;
            }
            C::SymmetricAboutLine {
                first,
                second,
                line,
            } => {
                self.require_distinct_points(*first, *second)?;
                self.validate_line_span(*line)?;
            }
            C::LineCircleTangency {
                line_contact,
                circle_contact,
                ..
            } => {
                if line_contact == circle_contact {
                    return invalid("constraint contact", "tangency contacts must be distinct");
                }
                let line = self.require_line_contact(*line_contact)?;
                let circle = self.require_circle_contact(*circle_contact)?;
                require_tangent_orientation(line)?;
                require_tangent_orientation(circle)?;
                self.validate_tangent_pair(line, circle)?;
            }
            C::CircleCircleTangency {
                first,
                second,
                mode,
                center_direction,
                ..
            } => {
                let first_curve = self.require_circle(*first)?;
                let second_curve = self.require_circle(*second)?;
                if first == second {
                    return invalid("constraint.definition", "circles must be distinct");
                }
                validate_unit_direction(*center_direction, "center_direction")?;
                let first_radius = self.curve_radius(first_curve)?;
                let second_radius = self.curve_radius(second_curve)?;
                let effective = match mode {
                    DocumentCircleTangencyMode::External => first_radius + second_radius,
                    DocumentCircleTangencyMode::Internal {
                        containment: DocumentCircleContainment::FirstContainsSecond,
                    } => first_radius - second_radius,
                    DocumentCircleTangencyMode::Internal {
                        containment: DocumentCircleContainment::SecondContainsFirst,
                    } => second_radius - first_radius,
                };
                finite_positive(effective, "circle tangency effective radius")?;
            }
            C::CircleArcTangency {
                circle_contact,
                arc_contact,
                ..
            } => {
                if circle_contact == arc_contact {
                    return invalid("constraint contact", "tangency contacts must be distinct");
                }
                let circle = self.require_circle_contact(*circle_contact)?;
                let arc = self.require_arc_contact(*arc_contact)?;
                require_tangent_orientation(circle)?;
                require_tangent_orientation(arc)?;
                self.validate_tangent_pair(circle, arc)?;
            }
            C::LineCurveTangency {
                line,
                curve_contact,
                ..
            } => {
                self.validate_line_span(*line)?;
                let contact = self.require_contact(*curve_contact)?;
                require_tangent_orientation(contact)?;
                let line_tangent = self.line_span_tangent(*line)?;
                let curve_tangent = self.contact_tangent(contact)?;
                let product = dot(line_tangent, curve_tangent);
                let valid = match contact
                    .tangent_orientation
                    .expect("orientation was required")
                {
                    TangentOrientation::Aligned => product > 0.0,
                    TangentOrientation::Opposed => product < 0.0,
                };
                if !valid {
                    return invalid(
                        "contact.tangent_orientation",
                        "selected orientation disagrees with line and curve tangents",
                    );
                }
            }
            C::CurveCurveContact {
                first_contact,
                second_contact,
            } => {
                if first_contact == second_contact {
                    return invalid("constraint contact", "curve contacts must be distinct");
                }
                for contact in [first_contact, second_contact] {
                    if self
                        .require_contact(*contact)?
                        .tangent_orientation
                        .is_some()
                    {
                        return invalid(
                            "constraint contact",
                            "contact-only curve pairs must not select tangent orientation",
                        );
                    }
                }
            }
            C::CurveCurveTangency {
                first_contact,
                second_contact,
            } => {
                if first_contact == second_contact {
                    return invalid("constraint contact", "tangency contacts must be distinct");
                }
                let first = self.require_contact(*first_contact)?;
                let second = self.require_contact(*second_contact)?;
                require_tangent_orientation(first)?;
                require_tangent_orientation(second)?;
                self.validate_tangent_pair(first, second)?;
            }
            C::CurveDirection {
                line,
                curve_contact,
                relation,
            } => {
                self.validate_line_span(*line)?;
                let contact = self.require_contact(*curve_contact)?;
                let line_direction = self.line_span_tangent(*line)?;
                let differential = self.contact_differential(contact)?;
                let selected = match relation {
                    DocumentCurveDirectionRelation::Tangent { orientation } => {
                        let sign = match orientation {
                            TangentOrientation::Aligned => 1.0,
                            TangentOrientation::Opposed => -1.0,
                        };
                        [
                            differential.unit_tangent.x * sign,
                            differential.unit_tangent.y * sign,
                        ]
                    }
                    DocumentCurveDirectionRelation::Normal { side } => {
                        let sign = match side {
                            DocumentCurveNormalSide::Left => 1.0,
                            DocumentCurveNormalSide::Right => -1.0,
                        };
                        [
                            differential.left_normal.x * sign,
                            differential.left_normal.y * sign,
                        ]
                    }
                };
                if dot(line_direction, selected) <= 0.0 {
                    return invalid(
                        "constraint.relation",
                        "selected curve direction disagrees with the directed line",
                    );
                }
            }
            C::EqualCurvature {
                first_contact,
                second_contact,
                relation,
            } => {
                if first_contact == second_contact {
                    return invalid("constraint contact", "curvature contacts must be distinct");
                }
                let first = self.contact_differential(self.require_contact(*first_contact)?)?;
                let second = self.contact_differential(self.require_contact(*second_contact)?)?;
                let signs_match = first.signed_curvature.is_sign_positive()
                    == second.signed_curvature.is_sign_positive();
                let branch_valid = match relation {
                    DocumentCurveCurvatureRelation::Signed => true,
                    DocumentCurveCurvatureRelation::MagnitudeSameSign => {
                        first.signed_curvature != 0.0
                            && second.signed_curvature != 0.0
                            && signs_match
                    }
                    DocumentCurveCurvatureRelation::MagnitudeOppositeSign => {
                        first.signed_curvature != 0.0
                            && second.signed_curvature != 0.0
                            && !signs_match
                    }
                };
                if !branch_valid {
                    return invalid(
                        "constraint.relation",
                        "current curvature signs disagree with the selected magnitude branch",
                    );
                }
            }
            C::EndpointContinuity {
                first_contact,
                second_contact,
                continuity,
            } => {
                if first_contact == second_contact {
                    return invalid("constraint contact", "continuity contacts must be distinct");
                }
                for contact in [first_contact, second_contact] {
                    let contact = self.require_contact(*contact)?;
                    if !matches!(
                        contact.neighborhood,
                        ContactNeighborhood::Start | ContactNeighborhood::End
                    ) {
                        return invalid(
                            "constraint endpoint",
                            "endpoint continuity requires explicit start/end contacts",
                        );
                    }
                }
                if let DocumentCurveContinuity::ParametricC2 {
                    first_rate,
                    second_rate,
                } = continuity
                {
                    finite_positive(*first_rate, "parametric C2 first rate")?;
                    finite_positive(*second_rate, "parametric C2 second rate")?;
                }
            }
        }
        Ok(())
    }

    fn validate_dimension_definition(
        &self,
        definition: &DocumentDimensionDefinition,
    ) -> Result<(), DocumentError> {
        use DocumentDimensionDefinition as D;
        let target = match definition {
            D::PointDistance {
                first,
                second,
                target,
            } => {
                self.require_distinct_points(*first, *second)?;
                *target
            }
            D::CurveLength { curve, target } => {
                self.validate_line_span(*curve)?;
                *target
            }
            D::Radius { curve, target } | D::Diameter { curve, target } => {
                self.require_radial_curve(*curve)?;
                *target
            }
            D::OrientedAngle {
                first,
                second,
                target,
                ..
            } => {
                self.validate_line_span(*first)?;
                self.validate_line_span(*second)?;
                if first == second {
                    return invalid("dimension.definition", "angle spans must be distinct");
                }
                *target
            }
        };
        let scalar = self.require_scalar(target)?;
        finite_positive(scalar.value, "dimension target")?;
        let unit = match definition {
            D::OrientedAngle { .. } => ScalarUnit::Angle,
            _ => ScalarUnit::Length,
        };
        require_scalar_role(scalar, unit, ScalarDomain::Positive, "dimension target")
    }

    fn validate_span(&self, span: CurveSpan) -> Result<&DesignCurve, DocumentError> {
        let curve = self
            .curve(span.curve)
            .ok_or_else(|| unknown("curve", span.curve.0))?;
        match &curve.definition {
            CurveDefinition::BSpline { span_ids, .. } | CurveDefinition::Nurbs { span_ids, .. } => {
                if !span_ids.contains(&span.segment) {
                    return invalid("curve span", "semantic span ID is outside the spline");
                }
            }
            _ => {
                let count = curve_segment_count(&curve.definition);
                if usize::try_from(span.segment).map_or(true, |index| index >= count) {
                    return invalid("curve span", "segment index is outside the curve");
                }
            }
        }
        Ok(curve)
    }

    fn validate_line_span(&self, span: CurveSpan) -> Result<&DesignCurve, DocumentError> {
        let curve = self.validate_span(span)?;
        if !matches!(
            curve.definition,
            CurveDefinition::Line { .. } | CurveDefinition::Polyline { .. }
        ) {
            return invalid("curve span", "expected a line or polyline segment");
        }
        Ok(curve)
    }

    fn require_point(&self, id: DesignPointId) -> Result<&DesignPoint, DocumentError> {
        self.point(id).ok_or_else(|| unknown("point", id.0))
    }

    fn require_scalar(&self, id: DesignScalarId) -> Result<&DesignScalar, DocumentError> {
        self.scalar(id).ok_or_else(|| unknown("scalar", id.0))
    }

    fn require_contact(&self, id: ContactId) -> Result<&ContactSlot, DocumentError> {
        self.contact(id).ok_or_else(|| unknown("contact", id.0))
    }

    fn require_distinct_points(
        &self,
        first: DesignPointId,
        second: DesignPointId,
    ) -> Result<(), DocumentError> {
        self.require_point(first)?;
        self.require_point(second)?;
        if first == second {
            invalid("point pair", "points must be distinct")
        } else {
            Ok(())
        }
    }

    fn require_distinct_noncoincident_points(
        &self,
        first: DesignPointId,
        second: DesignPointId,
    ) -> Result<(), DocumentError> {
        self.require_distinct_points(first, second)?;
        let first = self.require_point(first)?.position;
        let second = self.require_point(second)?.position;
        validate_direction(point_difference(first, second), "directed conic axis")
    }

    fn require_circle(&self, id: CurveId) -> Result<&DesignCurve, DocumentError> {
        let curve = self.curve(id).ok_or_else(|| unknown("curve", id.0))?;
        if matches!(curve.definition, CurveDefinition::Circle { .. }) {
            Ok(curve)
        } else {
            invalid("curve", "expected a circle")
        }
    }

    fn require_radial_curve(&self, id: CurveId) -> Result<&DesignCurve, DocumentError> {
        let curve = self.curve(id).ok_or_else(|| unknown("curve", id.0))?;
        if matches!(
            curve.definition,
            CurveDefinition::Circle { .. } | CurveDefinition::CircularArc { .. }
        ) {
            Ok(curve)
        } else {
            invalid("curve", "expected a circle or circular arc")
        }
    }

    fn require_line_contact(&self, id: ContactId) -> Result<&ContactSlot, DocumentError> {
        let contact = self.require_contact(id)?;
        self.validate_line_span(contact.curve)?;
        Ok(contact)
    }

    fn require_circle_contact(&self, id: ContactId) -> Result<&ContactSlot, DocumentError> {
        let contact = self.require_contact(id)?;
        self.require_circle(contact.curve.curve)?;
        Ok(contact)
    }

    fn require_arc_contact(&self, id: ContactId) -> Result<&ContactSlot, DocumentError> {
        let contact = self.require_contact(id)?;
        let curve = self
            .curve(contact.curve.curve)
            .ok_or_else(|| unknown("curve", contact.curve.curve.0))?;
        if matches!(curve.definition, CurveDefinition::CircularArc { .. }) {
            Ok(contact)
        } else {
            invalid("contact", "expected a circular-arc contact")
        }
    }

    fn curve_radius(&self, curve: &DesignCurve) -> Result<f64, DocumentError> {
        let (CurveDefinition::Circle { radius: scalar, .. }
        | CurveDefinition::CircularArc { radius: scalar, .. }) = curve.definition
        else {
            return invalid("curve", "expected a radial curve");
        };
        Ok(self.require_scalar(scalar)?.value)
    }

    fn validate_tangent_pair(
        &self,
        first: &ContactSlot,
        second: &ContactSlot,
    ) -> Result<(), DocumentError> {
        let first_orientation =
            first
                .tangent_orientation
                .ok_or_else(|| DocumentError::InvalidField {
                    field: "contact.tangent_orientation",
                    message: "tangency contact requires explicit orientation".into(),
                })?;
        if second.tangent_orientation != Some(first_orientation) {
            return invalid(
                "contact.tangent_orientation",
                "both tangency contacts must select the same relative orientation",
            );
        }
        let first_tangent = self.contact_tangent(first)?;
        let second_tangent = self.contact_tangent(second)?;
        let product = dot(first_tangent, second_tangent);
        let consistent = match first_orientation {
            TangentOrientation::Aligned => product > 0.0,
            TangentOrientation::Opposed => product < 0.0,
        };
        if consistent {
            Ok(())
        } else {
            invalid(
                "contact.tangent_orientation",
                "selected orientation disagrees with the current contact tangents",
            )
        }
    }

    fn contact_tangent(&self, contact: &ContactSlot) -> Result<[f64; 2], DocumentError> {
        let jet = self.evaluate_contact_jet(contact.id).map_err(|error| {
            contact_document_evaluation_error(contact.id, contact.curve.curve, error)
        })?;
        let tangent = [jet.first_derivative.x, jet.first_derivative.y];
        validate_direction(tangent, "contact tangent")?;
        Ok(tangent)
    }

    fn contact_differential(
        &self,
        contact: &ContactSlot,
    ) -> Result<geosolve_geometry::CurveDifferential2, DocumentError> {
        self.evaluate_contact_jet(contact.id)
            .map_err(|error| {
                contact_document_evaluation_error(contact.id, contact.curve.curve, error)
            })?
            .differential()
            .map_err(|source| DocumentError::ContactDifferential {
                contact: contact.id,
                source,
            })
    }

    fn line_span_tangent(&self, span: CurveSpan) -> Result<[f64; 2], DocumentError> {
        let curve = self.validate_line_span(span)?;
        let (start, end) = match &curve.definition {
            CurveDefinition::Line { start, end, .. } => (*start, *end),
            CurveDefinition::Polyline { points, closed, .. } => {
                let index = span.segment as usize;
                let next = if index + 1 == points.len() && *closed {
                    0
                } else {
                    index + 1
                };
                (points[index], points[next])
            }
            _ => return invalid("curve span", "expected a line segment"),
        };
        Ok(point_difference(
            self.require_point(start)?.position,
            self.require_point(end)?.position,
        ))
    }
}

fn curve_owned_scalars(definition: &CurveDefinition) -> Vec<DesignScalarId> {
    match definition {
        CurveDefinition::Circle { radius, .. } => vec![*radius],
        CurveDefinition::CircularArc {
            radius,
            start_angle,
            end_angle,
            ..
        } => vec![*radius, *start_angle, *end_angle],
        CurveDefinition::Ellipse {
            minor_axis_ratio, ..
        } => vec![*minor_axis_ratio],
        CurveDefinition::EllipticalArc {
            minor_axis_ratio,
            start_angle,
            end_angle,
            ..
        } => vec![*minor_axis_ratio, *start_angle, *end_angle],
        CurveDefinition::RationalQuadraticConic { middle_weight, .. } => vec![*middle_weight],
        CurveDefinition::ParabolaSegment {
            trim_start,
            trim_end,
            ..
        } => vec![*trim_start, *trim_end],
        CurveDefinition::HyperbolaSegment {
            semi_conjugate,
            trim_start,
            trim_end,
            ..
        } => vec![*semi_conjugate, *trim_start, *trim_end],
        CurveDefinition::Line { .. }
        | CurveDefinition::Polyline { .. }
        | CurveDefinition::QuadraticBezier { .. }
        | CurveDefinition::CubicBezier { .. }
        | CurveDefinition::BSpline { .. } => Vec::new(),
        CurveDefinition::Nurbs { weights, .. } => weights.clone(),
    }
}

enum DocumentConicGeometryError {
    Document(DocumentError),
    Definition(geosolve_geometry::ConicDefinitionError),
}

impl From<DocumentError> for DocumentConicGeometryError {
    fn from(error: DocumentError) -> Self {
        Self::Document(error)
    }
}

impl From<geosolve_geometry::ConicDefinitionError> for DocumentConicGeometryError {
    fn from(error: geosolve_geometry::ConicDefinitionError) -> Self {
        Self::Definition(error)
    }
}

fn document_curve_conic_geometry_error(
    error: DocumentConicGeometryError,
) -> DocumentCurveEvaluationError {
    match error {
        DocumentConicGeometryError::Document(error) => {
            DocumentCurveEvaluationError::Document(error)
        }
        DocumentConicGeometryError::Definition(error) => {
            DocumentCurveEvaluationError::ConicDefinition(error)
        }
    }
}

fn document_bspline_curve_error(
    curve: CurveId,
    error: DocumentCurveEvaluationError,
) -> DocumentError {
    match error {
        DocumentCurveEvaluationError::Document(error) => error,
        DocumentCurveEvaluationError::BSplineDefinition(source) => {
            DocumentError::BSplineDefinition { curve, source }
        }
        DocumentCurveEvaluationError::BSplineEvaluation(source) => {
            DocumentError::BSplineEvaluation { curve, source }
        }
        other => DocumentError::InvalidField {
            field: "curve",
            message: other.to_string(),
        },
    }
}

fn document_nurbs_curve_error(
    curve: CurveId,
    error: DocumentCurveEvaluationError,
) -> DocumentError {
    match error {
        DocumentCurveEvaluationError::Document(error) => error,
        DocumentCurveEvaluationError::BSplineDefinition(source) => {
            DocumentError::BSplineDefinition { curve, source }
        }
        DocumentCurveEvaluationError::BSplineEvaluation(source) => {
            DocumentError::BSplineEvaluation { curve, source }
        }
        DocumentCurveEvaluationError::NurbsDefinition(source) => {
            DocumentError::NurbsDefinition { curve, source }
        }
        DocumentCurveEvaluationError::NurbsEvaluation(source) => {
            DocumentError::NurbsEvaluation { curve, source }
        }
        other => DocumentError::InvalidField {
            field: "curve",
            message: other.to_string(),
        },
    }
}

fn document_query_conic_geometry_error(
    error: DocumentConicGeometryError,
) -> DocumentConicQueryError {
    match error {
        DocumentConicGeometryError::Document(error) => DocumentConicQueryError::Document(error),
        DocumentConicGeometryError::Definition(error) => DocumentConicQueryError::Definition(error),
    }
}

fn document_trim_projection_geometry_error(
    curve: CurveId,
    error: DocumentConicGeometryError,
) -> DocumentTrimProjectionError {
    match error {
        DocumentConicGeometryError::Document(error) => DocumentTrimProjectionError::Document(error),
        DocumentConicGeometryError::Definition(source) => {
            DocumentTrimProjectionError::ConicDefinition { curve, source }
        }
    }
}

fn angular_target_difference(
    curve: CurveId,
    center: [f64; 2],
    target: [f64; 2],
) -> Result<[f64; 2], DocumentTrimProjectionError> {
    let difference = [target[0] - center[0], target[1] - center[1]];
    if !difference.iter().all(|value| value.is_finite()) {
        return Err(DocumentTrimProjectionError::NonFiniteResult { curve });
    }
    if difference[0] == 0.0 && difference[1] == 0.0 {
        return Err(DocumentTrimProjectionError::AmbiguousCenterTarget { curve });
    }
    Ok(difference)
}

fn document_conic_geometry_document_error(
    curve: CurveId,
    error: DocumentConicGeometryError,
) -> DocumentError {
    match error {
        DocumentConicGeometryError::Document(error) => error,
        DocumentConicGeometryError::Definition(source) => {
            DocumentError::ConicDefinition { curve, source }
        }
    }
}

fn indexed_point(
    points: [geosolve_geometry::Point2<f64>; 2],
    index: u32,
) -> Option<geosolve_geometry::Point2<f64>> {
    usize::try_from(index)
        .ok()
        .and_then(|index| points.get(index).copied())
}

fn finite_query_point(
    curve: CurveId,
    point: geosolve_geometry::Point2<f64>,
) -> Result<[f64; 2], DocumentConicQueryError> {
    if point.coords.iter().all(|value| value.is_finite()) {
        Ok([point.x, point.y])
    } else {
        Err(DocumentConicQueryError::NonFiniteResult { curve })
    }
}

const fn conic_ratio_domain() -> ScalarDomain {
    ScalarDomain::Bounded {
        lower: f64::from_bits(1),
        upper: 1.0,
    }
}

const fn conic_weight_domain() -> ScalarDomain {
    ScalarDomain::Bounded {
        lower: crate::MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
        upper: f64::MAX,
    }
}

fn require_trim_scalar(scalar: &DesignScalar, field: &'static str) -> Result<(), DocumentError> {
    require_scalar_role(scalar, ScalarUnit::Parameter, ScalarDomain::Finite, field)
}

const fn is_conic_definition(definition: &CurveDefinition) -> bool {
    matches!(
        definition,
        CurveDefinition::Ellipse { .. }
            | CurveDefinition::EllipticalArc { .. }
            | CurveDefinition::RationalQuadraticConic { .. }
            | CurveDefinition::ParabolaSegment { .. }
            | CurveDefinition::HyperbolaSegment { .. }
    )
}

pub(crate) const fn document_hyperbola_branch(
    branch: DocumentHyperbolaBranch,
) -> geosolve_geometry::HyperbolaBranch {
    match branch {
        DocumentHyperbolaBranch::Positive => geosolve_geometry::HyperbolaBranch::Positive,
        DocumentHyperbolaBranch::Negative => geosolve_geometry::HyperbolaBranch::Negative,
    }
}

fn fresh_document_id() -> DocumentId {
    #[cfg(not(target_arch = "wasm32"))]
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    #[cfg(target_arch = "wasm32")]
    let timestamp = (u128::from(js_sys::Date::now().to_bits()) << 64)
        ^ u128::from(js_sys::Math::random().to_bits());
    let nonce = u128::from(DOCUMENT_NONCE.fetch_add(1, Ordering::Relaxed));
    let value = timestamp.rotate_left(32) ^ nonce;
    DocumentId(PersistentId(value.max(1)))
}

fn validate_contact(
    contact: &ContactSlot,
    scalar: &DesignScalar,
    bounded_winding_allowed: bool,
) -> Result<(), DocumentError> {
    let value = scalar.value;
    match contact.domain {
        ContactDomain::SupportingLine => {
            finite(value, "contact parameter")?;
            if contact.winding != 0 {
                return invalid("contact.winding", "supporting-line winding must be zero");
            }
        }
        ContactDomain::Bounded { lower, upper } => {
            finite(lower, "contact lower bound")?;
            finite(upper, "contact upper bound")?;
            if lower >= upper || !(lower..=upper).contains(&value) {
                return Err(DocumentError::ContactParameterOutOfDomain {
                    contact: contact.id,
                    value,
                    lower,
                    upper,
                });
            }
            if contact.winding != 0 && !bounded_winding_allowed {
                return invalid("contact.winding", "bounded contact winding must be zero");
            }
        }
        ContactDomain::Periodic { period } => {
            finite_positive(period, "contact period")?;
            if !(0.0..period).contains(&value) {
                return invalid(
                    "contact.parameter",
                    "periodic principal value must be in [0, period)",
                );
            }
        }
    }
    Ok(())
}

fn remap_bspline_contact_neighborhood(
    neighborhood: ContactNeighborhood,
    old_span: &geosolve_geometry::BSplineSpan,
    inserted_knot: f64,
    left_child: bool,
    parameter: f64,
) -> Result<ContactNeighborhood, DocumentError> {
    if parameter.to_bits() == 0.0f64.to_bits() {
        return Ok(ContactNeighborhood::Start);
    }
    if parameter.to_bits() == 1.0f64.to_bits() {
        return Ok(ContactNeighborhood::End);
    }
    match neighborhood {
        ContactNeighborhood::Interior => Ok(ContactNeighborhood::Interior),
        ContactNeighborhood::Local { lower, upper } => {
            let old_width = old_span.upper() - old_span.lower();
            let old_lower = old_width.mul_add(lower, old_span.lower());
            let old_upper = old_width.mul_add(upper, old_span.lower());
            let (child_lower, child_upper) = if left_child {
                (old_span.lower(), inserted_knot)
            } else {
                (inserted_knot, old_span.upper())
            };
            let child_width = child_upper - child_lower;
            let lower = ((old_lower - child_lower) / child_width).clamp(0.0, 1.0);
            let upper = ((old_upper - child_lower) / child_width).clamp(0.0, 1.0);
            if lower < parameter && parameter < upper {
                Ok(ContactNeighborhood::Local { lower, upper })
            } else {
                invalid(
                    "contact.neighborhood",
                    "refined local neighborhood no longer contains its contact",
                )
            }
        }
        ContactNeighborhood::Start | ContactNeighborhood::End => invalid(
            "contact.neighborhood",
            "endpoint neighborhood remapped to an interior child coordinate",
        ),
    }
}

fn contact_document_evaluation_error(
    contact: ContactId,
    curve: CurveId,
    error: DocumentCurveEvaluationError,
) -> DocumentError {
    match error {
        DocumentCurveEvaluationError::Document(source) => source,
        DocumentCurveEvaluationError::Curve(
            geosolve_geometry::CurveEvaluationError::Regularity(source),
        )
        | DocumentCurveEvaluationError::ConicEvaluation(
            geosolve_geometry::ConicEvaluationError::Curve(
                geosolve_geometry::CurveEvaluationError::Regularity(source),
            ),
        ) => DocumentError::ContactRegularity { contact, source },
        DocumentCurveEvaluationError::ConicDefinition(source) => {
            DocumentError::ConicDefinition { curve, source }
        }
        DocumentCurveEvaluationError::ConicEvaluation(source) => {
            DocumentError::ContactConicEvaluation { contact, source }
        }
        DocumentCurveEvaluationError::BSplineDefinition(source) => {
            DocumentError::BSplineDefinition { curve, source }
        }
        DocumentCurveEvaluationError::BSplineEvaluation(source) => {
            DocumentError::BSplineEvaluation { curve, source }
        }
        DocumentCurveEvaluationError::NurbsDefinition(source) => {
            DocumentError::NurbsDefinition { curve, source }
        }
        DocumentCurveEvaluationError::NurbsEvaluation(source) => {
            DocumentError::NurbsEvaluation { curve, source }
        }
        DocumentCurveEvaluationError::Curve(other) => DocumentError::InvalidField {
            field: "contact tangent",
            message: other.to_string(),
        },
    }
}

fn geometry_line_domain(
    domain: ContactDomain,
) -> Result<geosolve_geometry::CurveParameterDomain, DocumentError> {
    match domain {
        ContactDomain::SupportingLine => {
            Ok(geosolve_geometry::CurveParameterDomain::SupportingLine)
        }
        ContactDomain::Bounded { lower, upper } => {
            Ok(geosolve_geometry::CurveParameterDomain::Bounded { lower, upper })
        }
        ContactDomain::Periodic { .. } => invalid(
            "contact.domain",
            "a line or polyline contact cannot use a periodic domain",
        ),
    }
}

fn validate_contact_curve(
    contact: &ContactSlot,
    scalar: &DesignScalar,
    curve: &DesignCurve,
) -> Result<(), DocumentError> {
    match (&curve.definition, contact.domain) {
        (
            CurveDefinition::Line { .. } | CurveDefinition::Polyline { .. },
            ContactDomain::SupportingLine,
        ) => {
            if scalar.unit != ScalarUnit::Parameter || scalar.domain != ScalarDomain::Finite {
                return invalid(
                    "contact.parameter",
                    "supporting-line contact requires a finite parameter scalar",
                );
            }
        }
        (
            CurveDefinition::Line { .. }
            | CurveDefinition::Polyline { .. }
            | CurveDefinition::CircularArc { .. }
            | CurveDefinition::QuadraticBezier { .. }
            | CurveDefinition::CubicBezier { .. }
            | CurveDefinition::EllipticalArc { .. }
            | CurveDefinition::RationalQuadraticConic { .. }
            | CurveDefinition::ParabolaSegment { .. }
            | CurveDefinition::HyperbolaSegment { .. }
            | CurveDefinition::BSpline { .. }
            | CurveDefinition::Nurbs { .. },
            ContactDomain::Bounded { lower, upper },
        ) if is_unit_interval(lower, upper) => {
            if scalar.unit != ScalarUnit::Parameter
                || scalar.domain != (ScalarDomain::Bounded { lower, upper })
            {
                return invalid(
                    "contact.parameter",
                    "bounded contact requires a parameter scalar with the same domain",
                );
            }
        }
        (
            CurveDefinition::Circle { .. } | CurveDefinition::Ellipse { .. },
            ContactDomain::Periodic { period },
        ) if period.to_bits() == std::f64::consts::TAU.to_bits() => {
            if scalar.unit != ScalarUnit::Angle
                || scalar.domain != (ScalarDomain::Periodic { period })
            {
                return invalid(
                    "contact.parameter",
                    "periodic contact requires an angle scalar with the same domain",
                );
            }
        }
        _ => {
            return invalid(
                "contact.domain",
                "domain is incompatible with the selected curve family",
            );
        }
    }
    match (contact.domain, contact.neighborhood) {
        (
            ContactDomain::SupportingLine | ContactDomain::Periodic { .. },
            ContactNeighborhood::Interior,
        ) => {}
        (ContactDomain::Bounded { lower, .. }, ContactNeighborhood::Start)
            if scalar.value.to_bits() == lower.to_bits() => {}
        (ContactDomain::Bounded { upper, .. }, ContactNeighborhood::End)
            if scalar.value.to_bits() == upper.to_bits() => {}
        (ContactDomain::Bounded { lower, upper }, ContactNeighborhood::Interior)
            if scalar.value > lower && scalar.value < upper => {}
        (
            ContactDomain::Bounded {
                lower: domain_lower,
                upper: domain_upper,
            },
            ContactNeighborhood::Local { lower, upper },
        ) if lower.is_finite()
            && upper.is_finite()
            && lower >= domain_lower
            && lower < scalar.value
            && scalar.value < upper
            && upper <= domain_upper => {}
        _ => {
            return invalid(
                "contact.neighborhood",
                "selection does not match the contact parameter",
            );
        }
    }
    Ok(())
}

fn require_tangent_orientation(contact: &ContactSlot) -> Result<(), DocumentError> {
    if contact.tangent_orientation.is_some() {
        Ok(())
    } else {
        invalid(
            "contact.tangent_orientation",
            "tangency contact requires explicit orientation",
        )
    }
}

fn constraint_contacts(definition: &DocumentConstraintDefinition) -> Vec<ContactId> {
    match definition {
        DocumentConstraintDefinition::PointOnCurve { contact, .. } => vec![*contact],
        DocumentConstraintDefinition::LineCircleTangency {
            line_contact,
            circle_contact,
            ..
        } => vec![*line_contact, *circle_contact],
        DocumentConstraintDefinition::CircleArcTangency {
            circle_contact,
            arc_contact,
            ..
        } => vec![*circle_contact, *arc_contact],
        DocumentConstraintDefinition::LineCurveTangency { curve_contact, .. }
        | DocumentConstraintDefinition::CurveDirection { curve_contact, .. } => {
            vec![*curve_contact]
        }
        DocumentConstraintDefinition::CurveCurveContact {
            first_contact,
            second_contact,
        }
        | DocumentConstraintDefinition::CurveCurveTangency {
            first_contact,
            second_contact,
        }
        | DocumentConstraintDefinition::EqualCurvature {
            first_contact,
            second_contact,
            ..
        }
        | DocumentConstraintDefinition::EndpointContinuity {
            first_contact,
            second_contact,
            ..
        } => vec![*first_contact, *second_contact],
        _ => Vec::new(),
    }
}

#[allow(clippy::too_many_lines)]
fn curve_references_object(definition: &CurveDefinition, object: DocumentObjectId) -> bool {
    match (definition, object) {
        (CurveDefinition::Line { start, end, .. }, DocumentObjectId::Point(point)) => {
            *start == point || *end == point
        }
        (CurveDefinition::Polyline { points, .. }, DocumentObjectId::Point(point)) => {
            points.contains(&point)
        }
        (
            CurveDefinition::Circle { center, .. } | CurveDefinition::CircularArc { center, .. },
            DocumentObjectId::Point(point),
        ) => *center == point,
        (CurveDefinition::QuadraticBezier { controls }, DocumentObjectId::Point(point)) => {
            controls.contains(&point)
        }
        (CurveDefinition::CubicBezier { controls }, DocumentObjectId::Point(point)) => {
            controls.contains(&point)
        }
        (
            CurveDefinition::BSpline { controls, .. } | CurveDefinition::Nurbs { controls, .. },
            DocumentObjectId::Point(point),
        ) => controls.contains(&point),
        (
            CurveDefinition::Ellipse {
                center,
                major_axis_point,
                ..
            }
            | CurveDefinition::EllipticalArc {
                center,
                major_axis_point,
                ..
            },
            DocumentObjectId::Point(point),
        ) => *center == point || *major_axis_point == point,
        (
            CurveDefinition::RationalQuadraticConic { start, end, .. },
            DocumentObjectId::Point(point),
        ) => *start == point || *end == point,
        (
            CurveDefinition::ParabolaSegment { vertex, focus, .. },
            DocumentObjectId::Point(point),
        ) => *vertex == point || *focus == point,
        (
            CurveDefinition::HyperbolaSegment {
                center,
                transverse_axis_point,
                ..
            },
            DocumentObjectId::Point(point),
        ) => *center == point || *transverse_axis_point == point,
        (CurveDefinition::Circle { radius, .. }, DocumentObjectId::Scalar(scalar)) => {
            *radius == scalar
        }
        (
            CurveDefinition::CircularArc {
                radius,
                start_angle,
                end_angle,
                ..
            },
            DocumentObjectId::Scalar(scalar),
        ) => *radius == scalar || *start_angle == scalar || *end_angle == scalar,
        (
            CurveDefinition::Ellipse {
                minor_axis_ratio, ..
            },
            DocumentObjectId::Scalar(scalar),
        ) => *minor_axis_ratio == scalar,
        (
            CurveDefinition::EllipticalArc {
                minor_axis_ratio,
                start_angle,
                end_angle,
                ..
            },
            DocumentObjectId::Scalar(scalar),
        ) => *minor_axis_ratio == scalar || *start_angle == scalar || *end_angle == scalar,
        (
            CurveDefinition::RationalQuadraticConic { middle_weight, .. },
            DocumentObjectId::Scalar(scalar),
        ) => *middle_weight == scalar,
        (
            CurveDefinition::ParabolaSegment {
                trim_start,
                trim_end,
                ..
            },
            DocumentObjectId::Scalar(scalar),
        ) => *trim_start == scalar || *trim_end == scalar,
        (
            CurveDefinition::HyperbolaSegment {
                semi_conjugate,
                trim_start,
                trim_end,
                ..
            },
            DocumentObjectId::Scalar(scalar),
        ) => *semi_conjugate == scalar || *trim_start == scalar || *trim_end == scalar,
        (CurveDefinition::Nurbs { weights, .. }, DocumentObjectId::Scalar(scalar)) => {
            weights.contains(&scalar)
        }
        _ => false,
    }
}

fn contact_references_object(contact: &ContactSlot, object: DocumentObjectId) -> bool {
    match object {
        DocumentObjectId::Curve(curve) => contact.curve.curve == curve,
        DocumentObjectId::Scalar(scalar) => contact.parameter == scalar,
        _ => false,
    }
}

#[allow(clippy::too_many_lines)]
fn constraint_references_object(
    definition: &DocumentConstraintDefinition,
    object: DocumentObjectId,
) -> bool {
    match (definition, object) {
        (
            DocumentConstraintDefinition::FixedPoint { point, .. }
            | DocumentConstraintDefinition::FixedCoordinate { point, .. }
            | DocumentConstraintDefinition::PointOnCurve { point, .. }
            | DocumentConstraintDefinition::Midpoint { point, .. },
            DocumentObjectId::Point(selected),
        ) => *point == selected,
        (
            DocumentConstraintDefinition::Coincident { first, second }
            | DocumentConstraintDefinition::SymmetricAboutLine { first, second, .. },
            DocumentObjectId::Point(selected),
        ) => *first == selected || *second == selected,
        (
            DocumentConstraintDefinition::Horizontal { line }
            | DocumentConstraintDefinition::Vertical { line }
            | DocumentConstraintDefinition::Midpoint { line, .. }
            | DocumentConstraintDefinition::SymmetricAboutLine { line, .. }
            | DocumentConstraintDefinition::LineCurveTangency { line, .. }
            | DocumentConstraintDefinition::CurveDirection { line, .. },
            DocumentObjectId::Curve(selected),
        ) => line.curve == selected,
        (
            DocumentConstraintDefinition::Parallel { first, second }
            | DocumentConstraintDefinition::Perpendicular { first, second }
            | DocumentConstraintDefinition::EqualLength { first, second },
            DocumentObjectId::Curve(selected),
        ) => first.curve == selected || second.curve == selected,
        (
            DocumentConstraintDefinition::EqualRadius { first, second }
            | DocumentConstraintDefinition::CircleCircleTangency { first, second, .. },
            DocumentObjectId::Curve(selected),
        ) => *first == selected || *second == selected,
        (
            DocumentConstraintDefinition::PointOnCurve { contact, .. }
            | DocumentConstraintDefinition::LineCurveTangency {
                curve_contact: contact,
                ..
            }
            | DocumentConstraintDefinition::CurveDirection {
                curve_contact: contact,
                ..
            },
            DocumentObjectId::Contact(selected),
        ) => *contact == selected,
        (
            DocumentConstraintDefinition::LineCircleTangency {
                line_contact,
                circle_contact,
                ..
            },
            DocumentObjectId::Contact(selected),
        ) => *line_contact == selected || *circle_contact == selected,
        (
            DocumentConstraintDefinition::CircleArcTangency {
                circle_contact,
                arc_contact,
                ..
            },
            DocumentObjectId::Contact(selected),
        ) => *circle_contact == selected || *arc_contact == selected,
        (
            DocumentConstraintDefinition::CurveCurveContact {
                first_contact,
                second_contact,
            }
            | DocumentConstraintDefinition::CurveCurveTangency {
                first_contact,
                second_contact,
            }
            | DocumentConstraintDefinition::EqualCurvature {
                first_contact,
                second_contact,
                ..
            }
            | DocumentConstraintDefinition::EndpointContinuity {
                first_contact,
                second_contact,
                ..
            },
            DocumentObjectId::Contact(selected),
        ) => *first_contact == selected || *second_contact == selected,
        _ => false,
    }
}

fn dimension_references_object(
    definition: &DocumentDimensionDefinition,
    object: DocumentObjectId,
) -> bool {
    match (definition, object) {
        (
            DocumentDimensionDefinition::PointDistance { first, second, .. },
            DocumentObjectId::Point(point),
        ) => *first == point || *second == point,
        (
            DocumentDimensionDefinition::CurveLength { curve, .. },
            DocumentObjectId::Curve(selected),
        ) => curve.curve == selected,
        (
            DocumentDimensionDefinition::Radius { curve, .. }
            | DocumentDimensionDefinition::Diameter { curve, .. },
            DocumentObjectId::Curve(selected),
        ) => *curve == selected,
        (
            DocumentDimensionDefinition::OrientedAngle { first, second, .. },
            DocumentObjectId::Curve(selected),
        ) => first.curve == selected || second.curve == selected,
        (definition, DocumentObjectId::Scalar(scalar)) => dimension_target(definition) == scalar,
        _ => false,
    }
}

fn is_unit_interval(lower: f64, upper: f64) -> bool {
    lower.to_bits() == 0.0f64.to_bits() && upper.to_bits() == 1.0f64.to_bits()
}

fn validate_scalar_value(value: f64, domain: ScalarDomain) -> Result<(), DocumentError> {
    finite(value, "scalar value")?;
    match domain {
        ScalarDomain::Finite => Ok(()),
        ScalarDomain::Positive => finite_positive(value, "scalar value"),
        ScalarDomain::Bounded { lower, upper } => {
            finite(lower, "scalar lower bound")?;
            finite(upper, "scalar upper bound")?;
            if lower < upper && (lower..=upper).contains(&value) {
                Ok(())
            } else {
                invalid(
                    "scalar domain",
                    "value must lie in a nonempty bounded interval",
                )
            }
        }
        ScalarDomain::Periodic { period } => {
            finite_positive(period, "scalar period")?;
            if (0.0..period).contains(&value) {
                Ok(())
            } else {
                invalid("scalar domain", "periodic value must be in [0, period)")
            }
        }
    }
}

fn curve_segment_count(definition: &CurveDefinition) -> usize {
    match definition {
        CurveDefinition::Polyline { points, closed, .. } => {
            points.len().saturating_sub(1) + usize::from(*closed)
        }
        CurveDefinition::Line { .. }
        | CurveDefinition::Circle { .. }
        | CurveDefinition::CircularArc { .. }
        | CurveDefinition::QuadraticBezier { .. }
        | CurveDefinition::CubicBezier { .. }
        | CurveDefinition::Ellipse { .. }
        | CurveDefinition::EllipticalArc { .. }
        | CurveDefinition::RationalQuadraticConic { .. }
        | CurveDefinition::ParabolaSegment { .. }
        | CurveDefinition::HyperbolaSegment { .. } => 1,
        CurveDefinition::BSpline { span_ids, .. } | CurveDefinition::Nurbs { span_ids, .. } => {
            span_ids.len()
        }
    }
}

fn curve_scalars(definition: &CurveDefinition) -> Vec<DesignScalarId> {
    match definition {
        CurveDefinition::Line { .. }
        | CurveDefinition::Polyline { .. }
        | CurveDefinition::QuadraticBezier { .. }
        | CurveDefinition::CubicBezier { .. }
        | CurveDefinition::BSpline { .. } => Vec::new(),
        CurveDefinition::Nurbs { weights, .. } => weights.clone(),
        CurveDefinition::Circle { radius, .. } => vec![*radius],
        CurveDefinition::CircularArc {
            radius,
            start_angle,
            end_angle,
            ..
        } => vec![*radius, *start_angle, *end_angle],
        CurveDefinition::Ellipse {
            minor_axis_ratio, ..
        } => vec![*minor_axis_ratio],
        CurveDefinition::EllipticalArc {
            minor_axis_ratio,
            start_angle,
            end_angle,
            ..
        } => vec![*minor_axis_ratio, *start_angle, *end_angle],
        CurveDefinition::RationalQuadraticConic { middle_weight, .. } => vec![*middle_weight],
        CurveDefinition::ParabolaSegment {
            trim_start,
            trim_end,
            ..
        } => vec![*trim_start, *trim_end],
        CurveDefinition::HyperbolaSegment {
            semi_conjugate,
            trim_start,
            trim_end,
            ..
        } => vec![*semi_conjugate, *trim_start, *trim_end],
    }
}

const fn dimension_target(definition: &DocumentDimensionDefinition) -> DesignScalarId {
    match definition {
        DocumentDimensionDefinition::PointDistance { target, .. }
        | DocumentDimensionDefinition::CurveLength { target, .. }
        | DocumentDimensionDefinition::Radius { target, .. }
        | DocumentDimensionDefinition::Diameter { target, .. }
        | DocumentDimensionDefinition::OrientedAngle { target, .. } => *target,
    }
}

fn claim_scalar(
    used: &mut BTreeSet<DesignScalarId>,
    scalar: DesignScalarId,
) -> Result<(), DocumentError> {
    if used.insert(scalar) {
        Ok(())
    } else {
        invalid(
            "scalar identity",
            "each scalar must have one geometry, contact, or target owner",
        )
    }
}

fn require_scalar_role(
    scalar: &DesignScalar,
    unit: ScalarUnit,
    domain: ScalarDomain,
    field: &'static str,
) -> Result<(), DocumentError> {
    if scalar.unit == unit && scalar.domain == domain {
        Ok(())
    } else {
        invalid(
            field,
            "scalar unit or domain does not match its semantic role",
        )
    }
}

fn contact_total_value(contact: &ContactSlot, principal: f64) -> f64 {
    match contact.domain {
        ContactDomain::Periodic { period } => principal + f64::from(contact.winding) * period,
        ContactDomain::SupportingLine | ContactDomain::Bounded { .. } => principal,
    }
}

fn point_difference(first: [f64; 2], second: [f64; 2]) -> [f64; 2] {
    [second[0] - first[0], second[1] - first[1]]
}

pub(crate) fn document_arc_signed_sweep(
    start: f64,
    end: f64,
    sweep: DocumentArcSweep,
) -> Result<f64, DocumentError> {
    let magnitude = match sweep {
        DocumentArcSweep::CounterClockwise => (end - start).rem_euclid(std::f64::consts::TAU),
        DocumentArcSweep::Clockwise => (start - end).rem_euclid(std::f64::consts::TAU),
    };
    if !magnitude.is_finite() || magnitude == 0.0 {
        return invalid("arc sweep", "must be finite and nonzero");
    }
    Ok(match sweep {
        DocumentArcSweep::CounterClockwise => magnitude,
        DocumentArcSweep::Clockwise => -magnitude,
    })
}

fn validate_label(label: &str, field: &'static str) -> Result<(), DocumentError> {
    if label.trim().is_empty() {
        return invalid(field, "must not be empty");
    }
    if label.len() > MAX_LABEL_BYTES {
        return Err(DocumentError::ResourceLimit {
            resource: "label bytes",
            actual: label.len(),
            limit: MAX_LABEL_BYTES,
        });
    }
    Ok(())
}

fn finite(value: f64, field: &'static str) -> Result<(), DocumentError> {
    if value.is_finite() {
        Ok(())
    } else {
        invalid(field, "must be finite")
    }
}

fn finite_positive(value: f64, field: &'static str) -> Result<(), DocumentError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        invalid(field, "must be positive and finite")
    }
}

fn finite_pair(value: [f64; 2], field: &'static str) -> Result<(), DocumentError> {
    finite(value[0], field)?;
    finite(value[1], field)
}

fn validate_direction(value: [f64; 2], field: &'static str) -> Result<(), DocumentError> {
    finite_pair(value, field)?;
    let norm = value[0].hypot(value[1]);
    if norm.is_finite() && norm > 0.0 {
        Ok(())
    } else {
        invalid(field, "must be nonzero")
    }
}

fn validate_unit_direction(value: [f64; 2], field: &'static str) -> Result<(), DocumentError> {
    validate_direction(value, field)?;
    let norm = value[0].hypot(value[1]);
    if (norm - 1.0).abs() <= 64.0 * f64::EPSILON {
        Ok(())
    } else {
        invalid(field, "must be normalized")
    }
}

fn normalized_vector(value: [f64; 2]) -> Result<[f64; 2], DocumentError> {
    validate_direction(value, "branch direction")?;
    let norm = value[0].hypot(value[1]);
    Ok([value[0] / norm, value[1] / norm])
}

fn normalized_direction(first: [f64; 2], second: [f64; 2]) -> Result<[f64; 2], DocumentError> {
    let direction = [second[0] - first[0], second[1] - first[1]];
    let norm = direction[0].hypot(direction[1]);
    if norm.is_finite() && norm > 0.0 {
        Ok([direction[0] / norm, direction[1] / norm])
    } else {
        invalid("curve geometry", "segment must be finite and nondegenerate")
    }
}

fn dot(first: [f64; 2], second: [f64; 2]) -> f64 {
    first[0] * second[0] + first[1] * second[1]
}

fn insert_unique(ids: &mut BTreeSet<PersistentId>, id: PersistentId) -> Result<(), DocumentError> {
    if ids.insert(id) {
        Ok(())
    } else {
        Err(DocumentError::DuplicateId(id))
    }
}

fn unknown(kind: &'static str, id: PersistentId) -> DocumentError {
    DocumentError::UnknownId { kind, id }
}

fn invalid<T>(field: &'static str, message: impl Into<String>) -> Result<T, DocumentError> {
    Err(DocumentError::InvalidField {
        field,
        message: message.into(),
    })
}

fn retain_remove<T>(values: &mut Vec<T>, predicate: impl Fn(&T) -> bool) -> bool {
    let before = values.len();
    values.retain(|value| !predicate(value));
    values.len() != before
}

const fn object_persistent(object: DocumentObjectId) -> PersistentId {
    match object {
        DocumentObjectId::Point(id) => id.0,
        DocumentObjectId::Scalar(id) => id.0,
        DocumentObjectId::Curve(id) => id.0,
        DocumentObjectId::Contact(id) => id.0,
        DocumentObjectId::Constraint(id) => id.0,
        DocumentObjectId::Dimension(id) => id.0,
    }
}
