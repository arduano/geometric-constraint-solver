use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

use geosolve_core::{OperationCheckpoint, OperationController, OperationWorkCounter};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// Current on-disk sketch-document schema.
pub const SKETCH_DOCUMENT_VERSION: u32 = 4;
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
/// Defensive bound for one immutable host-configuration activation input.
pub const MAX_ACTIVATION_OVERRIDES: usize = MAX_DOCUMENT_OBJECTS;
/// Defensive bound for persistent host-parameter declarations and associations.
pub const MAX_DOCUMENT_PARAMETERS: usize = MAX_DOCUMENT_OBJECTS;
/// Defensive bound for persistent external-reference declarations.
pub const MAX_EXTERNAL_BINDINGS: usize = MAX_DOCUMENT_OBJECTS;
/// Defensive bound for spline curves retained by lifecycle identity high-water.
pub const MAX_PERSISTENT_SPLINE_SPAN_CURSORS: usize = MAX_DOCUMENT_OBJECTS;

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

fn validate_external_binding_shape(
    kind: ExternalFeatureKindV1,
    topology: Option<ExternalTopologyDigest>,
) -> Result<(), DocumentError> {
    match (kind, topology) {
        (ExternalFeatureKindV1::Point, None) | (ExternalFeatureKindV1::LineSegment, Some(_)) => {
            Ok(())
        }
        (ExternalFeatureKindV1::Point, Some(_)) => invalid(
            "external binding topology",
            "point bindings must not declare span topology",
        ),
        (ExternalFeatureKindV1::LineSegment, None) => invalid(
            "external binding topology",
            "line-segment bindings require stable span topology",
        ),
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
typed_id!(DocumentParameterId, "Persistent host-parameter identity.");
typed_id!(
    DocumentExternalBindingId,
    "Persistent document-local external-reference binding identity."
);
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
    #[error("supported sketch-v4 encoding cannot represent non-default M41 state")]
    UnsupportedM41State,
    #[error("supported sketch-v4 encoding cannot represent non-default M42 state")]
    UnsupportedM42State,
    #[error("supported sketch-v4 encoding cannot represent non-default M43 state")]
    UnsupportedM43State,
    #[error("supported sketch-v4 encoding cannot represent M58 operation trim topology")]
    UnsupportedM58State,
    #[error("supported sketch-v4 encoding cannot represent M71 retained planar relations")]
    UnsupportedM71State,
    #[error("supported sketch-v4 encoding cannot represent M74 datum relations")]
    UnsupportedM74State,
    #[error("supported sketch-v4 encoding cannot represent M80 profile-offset dimensions")]
    UnsupportedM80State,
    #[error("activation revision {actual} is not newer than retained revision {retained}")]
    StaleActivationRevision { actual: u64, retained: u64 },
    #[error("activation input contains duplicate element {0:?}")]
    DuplicateActivationElement(DocumentElementId),
    #[error("activation digest does not match the canonical activation payload")]
    ActivationDigestMismatch,
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

/// Stable identity of one selected-curve configuration control.
///
/// Controls are transient interaction affordances. They do not add persistent points,
/// constraint operands, snapping anchors, or serialized document state.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DocumentCurveControlId {
    pub curve: CurveId,
    pub kind: DocumentCurveControlKind,
}

/// Family-local role of one selected-curve configuration control.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum DocumentCurveControlKind {
    Center,
    StartPoint,
    EndPoint,
    ControlPoint { ordinal: u32 },
    Radius,
    TrimStart,
    TrimEnd,
    MajorAxisPoint,
    MinorAxis,
    RationalMiddle,
    Vertex,
    Focus,
    TransverseAxisPoint,
    ConjugateAxis,
}

/// Coordinate interpretation of the middle control of a rational quadratic conic.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DocumentRationalConicControlMode {
    /// Conventional Euclidean control `P1 = Qh / w`, valid only when `w != 0`.
    Euclidean,
    /// Raw homogeneous vector `Qh`, used explicitly when `w == 0`.
    Projective,
}

/// One complete atomic rational-quadratic middle-control configuration.
///
/// The persistent definition continues to store `(Qh, w)`. Euclidean input is converted to
/// `Qh = w * P1`; projective input is deliberately restricted to the zero-weight mode.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum DocumentRationalConicControl {
    Euclidean {
        middle: [f64; 2],
        weight: f64,
    },
    Projective {
        weighted_middle: [f64; 2],
        weight: f64,
    },
}

impl DocumentRationalConicControl {
    #[must_use]
    pub const fn mode(self) -> DocumentRationalConicControlMode {
        match self {
            Self::Euclidean { .. } => DocumentRationalConicControlMode::Euclidean,
            Self::Projective { .. } => DocumentRationalConicControlMode::Projective,
        }
    }

    #[must_use]
    pub const fn weight(self) -> f64 {
        match self {
            Self::Euclidean { weight, .. } | Self::Projective { weight, .. } => weight,
        }
    }
}

/// Persistent target owned by one transient curve control.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum DocumentCurveControlTarget {
    Point(DesignPointId),
    Scalar(DesignScalarId),
    RationalMiddle {
        weight: DesignScalarId,
        mode: DocumentRationalConicControlMode,
    },
}

/// Typed reason that a visible curve control cannot accept direct manipulation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum DocumentCurveControlWithholdingReason {
    InactiveCurve,
    AssociativeFilletOutput,
    HostParameterOwned,
    GaugeOwned,
    DrivingDimensionOwned,
    EqualRadiusOwned,
}

/// Editability of one visible selected-curve control.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DocumentCurveControlAvailability {
    Editable,
    ReadOnly(DocumentCurveControlWithholdingReason),
}

/// One transient, finite selected-curve control from accepted document geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DocumentCurveControl {
    pub id: DocumentCurveControlId,
    pub position: [f64; 2],
    pub target: DocumentCurveControlTarget,
    pub availability: DocumentCurveControlAvailability,
}

/// Inverse-mapped configuration edit produced from one curve-control target.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum DocumentCurveControlProjection {
    Point {
        point: DesignPointId,
        position: [f64; 2],
    },
    Scalar {
        scalar: DesignScalarId,
        value: f64,
    },
    RationalMiddle {
        curve: CurveId,
        control: DocumentRationalConicControl,
    },
}

/// Typed failure to enumerate or inverse-project selected-curve controls.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DocumentCurveControlError {
    #[error(transparent)]
    Document(#[from] DocumentError),
    #[error(transparent)]
    Evaluation(#[from] DocumentCurveEvaluationError),
    #[error(transparent)]
    ConicQuery(#[from] DocumentConicQueryError),
    #[error(transparent)]
    TrimProjection(#[from] DocumentTrimProjectionError),
    #[error("curve-control target for {control:?} must be finite")]
    NonFiniteTarget { control: DocumentCurveControlId },
    #[error("curve-control geometry for {control:?} is not finitely representable")]
    NonFiniteResult { control: DocumentCurveControlId },
    #[error("curve {curve} has no configuration control {kind:?}")]
    UnknownControl {
        curve: CurveId,
        kind: DocumentCurveControlKind,
    },
    #[error("curve control {control:?} is read-only: {reason:?}")]
    ReadOnly {
        control: DocumentCurveControlId,
        reason: DocumentCurveControlWithholdingReason,
    },
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
    #[error(
        "trim-endpoint projection for curve {curve} would make {endpoint:?} cross the opposite endpoint"
    )]
    CrossesOppositeEndpoint {
        curve: CurveId,
        endpoint: FeatureEndpoint,
    },
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

/// Persistent principal parameter and explicit traversal winding for one trim boundary.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentTrimParameter {
    pub parameter: f64,
    pub winding: i32,
}

/// Persistent provenance of one visible-interval boundary.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DocumentTrimBoundary {
    Fixed(DocumentTrimParameter),
    FilletContact {
        owner: DocumentConstraintId,
        contact: ContactId,
    },
    /// Boundary owned by an ordinary point-on-curve constraint emitted by a
    /// public equation-free construction transaction.
    ConstraintContact {
        owner: DocumentConstraintId,
        contact: ContactId,
    },
}

/// Equation-free visible interval over immutable curve support.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentCurveTrimView {
    pub support: CurveSpan,
    pub start: DocumentTrimBoundary,
    pub end: DocumentTrimBoundary,
}

/// Resolved unwrapped visible interval together with its persistent boundary provenance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DocumentVisibleCurveInterval {
    pub support: CurveSpan,
    pub start: f64,
    pub end: f64,
    pub start_boundary: DocumentTrimBoundary,
    pub end_boundary: DocumentTrimBoundary,
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

/// Capability-specific point operand with no coordinate-based fallback.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DocumentPointRef {
    Point { point: DesignPointId },
    Center(DocumentCenterRef),
    Endpoint(DocumentEndpointRef),
    Control(DocumentControlRef),
    Focus { curve: CurveId, index: u32 },
    FixedCurveLocation { contact: ContactId },
}

/// Capability-specific reference to a curve's stored semantic center.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentCenterRef {
    pub curve: CurveId,
}

/// Capability-specific reference to one non-periodic curve endpoint.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentEndpointRef {
    pub curve: CurveId,
    pub endpoint: FeatureEndpoint,
}

/// Capability-specific reference to one stored point control.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentControlRef {
    pub curve: CurveId,
    pub control: DesignPointId,
}

/// Explicit orientation of a direction or supporting-line operand.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentDirectionSense {
    Forward,
    Reverse,
}

/// Persistent directed supporting-line operand.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentLineSupportRef {
    pub span: CurveSpan,
    pub direction: DocumentDirectionSense,
}

/// Closed feature family expected by one immutable external binding.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalFeatureKindV1 {
    Point,
    LineSegment,
}

/// Stable host-supplied topology identity for one directed external span.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct ExternalTopologyDigest([u8; 32]);

impl ExternalTopologyDigest {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Persistent local declaration of an immutable external feature.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentExternalBinding {
    pub id: DocumentExternalBindingId,
    pub label: String,
    pub expected_kind: ExternalFeatureKindV1,
    pub expected_topology: Option<ExternalTopologyDigest>,
}

/// Explicit external point operand. It never denotes a native point or runtime variable.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentExternalPointRef {
    pub binding: DocumentExternalBindingId,
}

/// Explicit directed external line-support operand.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentExternalLineSupportRef {
    pub binding: DocumentExternalBindingId,
    pub direction: DocumentDirectionSense,
}

/// Persistent curve-span operand with explicit traversal winding.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentCurveSpanRef {
    pub span: CurveSpan,
    pub winding: i32,
}

/// Closed direction operand. Branch selection is serialized in every variant.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DocumentDirectionRef {
    CurveAxis {
        curve: CurveId,
        direction: DocumentDirectionSense,
    },
    LineSupport(DocumentLineSupportRef),
    CurveTangent {
        contact: ContactId,
        direction: DocumentDirectionSense,
    },
    CurveNormal {
        contact: ContactId,
        side: DocumentCurveNormalSide,
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
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
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

/// One complete explicit branch update for a persistent contact.
///
/// Unlike [`ContactStateEdit`], this form may change the selected semantic span
/// and parameter-domain topology while retaining the contact and parameter IDs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactBranchEdit {
    pub contact: ContactId,
    pub curve: CurveSpan,
    pub domain: ContactDomain,
    pub value: f64,
    pub winding: i32,
    pub neighborhood: ContactNeighborhood,
    pub tangent_orientation: Option<TangentOrientation>,
}

/// Intrinsic immutable reference geometry present in every Cartesian sketch.
///
/// Datums have no persistent object identity, solver variables, or history of
/// their own. Persistent constraints may refer to their fixed semantics.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SketchDatum {
    Origin,
    XAxis,
    YAxis,
}

impl SketchDatum {
    /// Returns the Cartesian axis represented by an axis datum.
    #[must_use]
    pub const fn coordinate_axis(self) -> Option<DocumentCoordinateAxis> {
        match self {
            Self::Origin => None,
            Self::XAxis => Some(DocumentCoordinateAxis::X),
            Self::YAxis => Some(DocumentCoordinateAxis::Y),
        }
    }
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

/// Explicit correspondence between source and target line endpoints.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentLineOffsetOrientation {
    Same,
    Reversed,
}

/// Explicit correspondence between line-parent contacts and fillet arc endpoints.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentFilletEndpointOrder {
    FirstThenSecond,
    SecondThenFirst,
}

/// Visible parent endpoint owned by one generic fillet contact.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentFilletTrimEndpoint {
    Start,
    End,
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
    /// Constrains one stored point to the intrinsic sketch origin.
    CoincidentWithOrigin {
        point: DesignPointId,
    },
    /// Constrains one stored point to the intrinsic Cartesian datum axis.
    PointOnDatumAxis {
        point: DesignPointId,
        axis: DocumentCoordinateAxis,
    },
    Coincident {
        first: DesignPointId,
        second: DesignPointId,
    },
    ExternalPointCoincident {
        point: DesignPointId,
        external: DocumentExternalPointRef,
    },
    Horizontal {
        line: CurveSpan,
    },
    Vertical {
        line: CurveSpan,
    },
    HorizontalPoints {
        first: DesignPointId,
        second: DesignPointId,
    },
    VerticalPoints {
        first: DesignPointId,
        second: DesignPointId,
    },
    HorizontalPointToMidpoint {
        point: DesignPointId,
        line: CurveSpan,
    },
    VerticalPointToMidpoint {
        point: DesignPointId,
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
    ExternalLineCollinear {
        line: DocumentLineSupportRef,
        external: DocumentExternalLineSupportRef,
    },
    /// Constrains one affine support to the intrinsic Cartesian datum axis.
    CollinearWithDatumAxis {
        line: DocumentLineSupportRef,
        axis: DocumentCoordinateAxis,
    },
    Concentric {
        first: DocumentCenterRef,
        second: DocumentCenterRef,
    },
    Collinear {
        first: DocumentLineSupportRef,
        second: DocumentLineSupportRef,
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
    /// Constrains two stored points to be reflections across an intrinsic Cartesian datum axis.
    SymmetricAboutDatumAxis {
        first: DesignPointId,
        second: DesignPointId,
        axis: DocumentCoordinateAxis,
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
    LineLineFillet {
        arc: CurveId,
        first_contact: ContactId,
        first_side: DocumentCurveNormalSide,
        second_contact: ContactId,
        second_side: DocumentCurveNormalSide,
        endpoint_order: DocumentFilletEndpointOrder,
    },
    CurveCurveFillet {
        arc: CurveId,
        first_contact: ContactId,
        first_side: DocumentCurveNormalSide,
        first_trim_endpoint: DocumentFilletTrimEndpoint,
        second_contact: ContactId,
        second_side: DocumentCurveNormalSide,
        second_trim_endpoint: DocumentFilletTrimEndpoint,
        endpoint_order: DocumentFilletEndpointOrder,
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

/// Explicit traversal of one persistent profile-offset support span.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentOffsetTraversal {
    Forward,
    Reverse,
}

/// One exact source or target support plus its retained traversal.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentDirectedProfileOffsetCurve {
    pub curve: CurveSpan,
    pub traversal: DocumentOffsetTraversal,
}

/// One source support and its existing same-family target support.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentProfileOffsetEdgePair {
    pub source: DocumentDirectedProfileOffsetCurve,
    pub target: DocumentDirectedProfileOffsetCurve,
}

/// Retained non-tangent turn at one ordered junction.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentProfileOffsetTurn {
    Left,
    Right,
}

/// Exact persistent source-junction ownership.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum DocumentProfileOffsetJunctionOwner {
    SharedPoint(DesignPointId),
    Constraint(DocumentConstraintId),
}

/// Retained local branch at one ordered source/target junction.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DocumentProfileOffsetJunctionBranch {
    Miter { turn: DocumentProfileOffsetTurn },
    Tangent,
}

/// One ordered junction with exact source ownership and an explicit branch.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentProfileOffsetJunction {
    pub source_owner: DocumentProfileOffsetJunctionOwner,
    pub target_owner: DocumentProfileOffsetJunctionOwner,
    pub branch: DocumentProfileOffsetJunctionBranch,
}

/// One closed, material-left profile loop.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentProfileOffsetLoop {
    pub edges: Vec<DocumentProfileOffsetEdgePair>,
    pub junctions: Vec<DocumentProfileOffsetJunction>,
}

/// One ordered open profile chain.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentProfileOffsetChain {
    pub edges: Vec<DocumentProfileOffsetEdgePair>,
    pub junctions: Vec<DocumentProfileOffsetJunction>,
    pub start_terminal: DocumentProfileOffsetTerminalPolicy,
    pub end_terminal: DocumentProfileOffsetTerminalPolicy,
}

/// Explicit endpoint policy retained by a topology-preserving open-chain offset.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentProfileOffsetTerminalPolicy {
    NormalTranslation,
}

/// Material-side direction for a persistent closed-face offset.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentFaceOffsetDirection {
    Outward,
    Inward,
}

/// Exact source/target topology retained by one profile-offset dimension.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DocumentProfileOffsetOperand {
    Face {
        direction: DocumentFaceOffsetDirection,
        outer: DocumentProfileOffsetLoop,
        holes: Vec<DocumentProfileOffsetLoop>,
    },
    OpenChain {
        side: DocumentLineSide,
        chain: DocumentProfileOffsetChain,
    },
}

/// Current persistent dimension-definition set.
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
    SupportingLineOffset {
        source: CurveSpan,
        target_segment: CurveSpan,
        target: DesignScalarId,
        side: DocumentLineSide,
        orientation: DocumentLineOffsetOrientation,
    },
    ExactTranslatedSegmentOffset {
        source: CurveSpan,
        target_segment: CurveSpan,
        target: DesignScalarId,
        side: DocumentLineSide,
        orientation: DocumentLineOffsetOrientation,
    },
    ProfileOffset {
        target: DesignScalarId,
        operand: DocumentProfileOffsetOperand,
    },
}

/// Persistent identities created by one atomic profile-offset declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DocumentProfileOffsetIds {
    pub target: DesignScalarId,
    pub dimension: DocumentDimensionId,
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

/// Closed canonical kind of one host-owned parameter.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentParameterKind {
    Length,
    Angle,
    Dimensionless,
    Activation,
}

/// Persistent declaration of one host-owned parameter identity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentParameter {
    pub id: DocumentParameterId,
    pub label: String,
    pub kind: DocumentParameterKind,
}

/// Typed persistent target supplied by one host input parameter.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", content = "target", rename_all = "snake_case")]
pub enum DocumentParameterTarget {
    DrivingDimension(DocumentDimensionId),
    /// One deliberately declared dimensionless runtime scalar property.
    DimensionlessFixedScalar(crate::semantic::DocumentScalarPropertyRef),
    Activation(DocumentElementId),
}

/// Persistent parameter-to-target association. Its pair is its stable identity.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentParameterBinding {
    pub parameter: DocumentParameterId,
    pub target: DocumentParameterTarget,
}

/// Persistent declaration of one reference-dimension output proposal.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentParameterOutput {
    pub parameter: DocumentParameterId,
    pub dimension: DocumentDimensionId,
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
    Parameter(DocumentParameterId),
    ExternalBinding(DocumentExternalBindingId),
}

/// Any persistent sketch-document element that application state may reference.
///
/// This is a semantic identity seam only. It never lowers to a runtime or core ID.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
#[non_exhaustive]
pub enum DocumentElementId {
    Document(DocumentId),
    Point(DesignPointId),
    Scalar(DesignScalarId),
    Curve(CurveId),
    Contact(ContactId),
    Constraint(DocumentConstraintId),
    Dimension(DocumentDimensionId),
    Parameter(DocumentParameterId),
    ExternalBinding(DocumentExternalBindingId),
    Source(DocumentSourceId),
}

/// Closed profile eligibility role for persistent sketch geometry.
///
/// Both roles remain ordinary lowerable and constrainable geometry. The role changes
/// only default profile eligibility.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometryRole {
    #[default]
    Profile,
    Construction,
}

/// One curve-scoped profile/construction role change in an atomic batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeometryRoleEdit {
    pub curve: CurveId,
    pub role: GeometryRole,
}

impl GeometryRoleEdit {
    #[must_use]
    pub const fn new(curve: CurveId, role: GeometryRole) -> Self {
        Self { curve, role }
    }
}

/// One explicit host-configuration activity decision.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "state", content = "element", rename_all = "snake_case")]
pub enum HostActivationOverride {
    Inactive(DocumentElementId),
    /// Reserved M43 seam. M41 stores no external geometry snapshot.
    UnavailableExternalReference(DocumentElementId),
}

impl HostActivationOverride {
    #[must_use]
    pub const fn element(self) -> DocumentElementId {
        match self {
            Self::Inactive(element) | Self::UnavailableExternalReference(element) => element,
        }
    }
}

/// Canonical deterministic identity of an immutable activation payload.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct ActivationDigest([u8; 32]);

impl ActivationDigest {
    #[must_use]
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Immutable host-configuration activation payload consumed by one document state.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HostConfigurationActivation {
    revision: u64,
    digest: ActivationDigest,
    overrides: Vec<HostActivationOverride>,
}

impl HostConfigurationActivation {
    /// Builds, canonicalizes, bounds, and digests an immutable activation payload.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero revision, excessive or duplicate overrides.
    pub fn new(
        revision: u64,
        overrides: Vec<HostActivationOverride>,
    ) -> Result<Self, DocumentError> {
        if revision == 0 {
            return invalid("activation revision", "must be positive");
        }
        if overrides.len() > MAX_ACTIVATION_OVERRIDES {
            return Err(DocumentError::ResourceLimit {
                resource: "activation overrides",
                actual: overrides.len(),
                limit: MAX_ACTIVATION_OVERRIDES,
            });
        }
        let mut overrides = overrides;
        overrides.sort_by_key(|entry| canonical_element_key(entry.element()));
        for pair in overrides.windows(2) {
            if pair[0].element() == pair[1].element() {
                return Err(DocumentError::DuplicateActivationElement(pair[0].element()));
            }
        }
        let digest = activation_digest(revision, &overrides);
        Ok(Self {
            revision,
            digest,
            overrides,
        })
    }

    /// Restores a payload only when its claimed canonical digest is exact.
    ///
    /// # Errors
    ///
    /// Returns an error when payload validation fails or the digest does not match.
    pub fn from_digest(
        revision: u64,
        digest: ActivationDigest,
        overrides: Vec<HostActivationOverride>,
    ) -> Result<Self, DocumentError> {
        let value = Self::new(revision, overrides)?;
        if value.digest != digest {
            return Err(DocumentError::ActivationDigestMismatch);
        }
        Ok(value)
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub const fn digest(&self) -> ActivationDigest {
        self.digest
    }

    #[must_use]
    pub fn overrides(&self) -> &[HostActivationOverride] {
        &self.overrides
    }
}

/// Closed explanation for one derived inactive document element.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InactivityReason {
    UserSuppressed,
    HostConfigurationInactive,
    UnavailableDependency { dependency: DocumentElementId },
    UnavailableExternalReference,
}

/// One canonical effective-activity entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DocumentElementActivity {
    pub element: DocumentElementId,
    pub reason: Option<InactivityReason>,
}

impl DocumentElementActivity {
    #[must_use]
    pub const fn is_active(self) -> bool {
        self.reason.is_none()
    }
}

/// Immutable deterministic dependency-closure result used by all M41 consumers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectiveActivity {
    activation_revision: u64,
    activation_digest: ActivationDigest,
    elements: Vec<DocumentElementActivity>,
}

impl EffectiveActivity {
    #[must_use]
    pub const fn activation_revision(&self) -> u64 {
        self.activation_revision
    }

    #[must_use]
    pub const fn activation_digest(&self) -> ActivationDigest {
        self.activation_digest
    }

    #[must_use]
    pub fn elements(&self) -> &[DocumentElementActivity] {
        &self.elements
    }

    #[must_use]
    pub fn reason(&self, element: impl Into<DocumentElementId>) -> Option<InactivityReason> {
        let element = element.into();
        self.elements
            .binary_search_by_key(&canonical_element_key(element), |entry| {
                canonical_element_key(entry.element)
            })
            .ok()
            .and_then(|index| self.elements[index].reason)
    }

    #[must_use]
    pub fn is_active(&self, element: impl Into<DocumentElementId>) -> bool {
        self.reason(element).is_none()
    }
}

impl DocumentElementId {
    #[must_use]
    pub const fn persistent_id(self) -> PersistentId {
        match self {
            Self::Document(id) => id.0,
            Self::Point(id) => id.0,
            Self::Scalar(id) => id.0,
            Self::Curve(id) => id.0,
            Self::Contact(id) => id.0,
            Self::Constraint(id) => id.0,
            Self::Dimension(id) => id.0,
            Self::Parameter(id) => id.0,
            Self::ExternalBinding(id) => id.0,
            Self::Source(id) => id.0,
        }
    }

    #[must_use]
    pub const fn kind(self) -> &'static str {
        match self {
            Self::Document(_) => "document",
            Self::Point(_) => "point",
            Self::Scalar(_) => "scalar",
            Self::Curve(_) => "curve",
            Self::Contact(_) => "contact",
            Self::Constraint(_) => "constraint",
            Self::Dimension(_) => "dimension",
            Self::Parameter(_) => "parameter",
            Self::ExternalBinding(_) => "external binding",
            Self::Source(_) => "source",
        }
    }
}

macro_rules! element_from_id {
    ($id:ty, $variant:ident) => {
        impl From<$id> for DocumentElementId {
            fn from(value: $id) -> Self {
                Self::$variant(value)
            }
        }
    };
}

element_from_id!(DocumentId, Document);
element_from_id!(DesignPointId, Point);
element_from_id!(DesignScalarId, Scalar);
element_from_id!(CurveId, Curve);
element_from_id!(ContactId, Contact);
element_from_id!(DocumentConstraintId, Constraint);
element_from_id!(DocumentDimensionId, Dimension);
element_from_id!(DocumentParameterId, Parameter);
element_from_id!(DocumentExternalBindingId, ExternalBinding);
element_from_id!(DocumentSourceId, Source);

/// Persistent owner of one document source/audit identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DocumentSourceOwner {
    Constraint(DocumentConstraintId),
    Dimension(DocumentDimensionId),
}

/// Read-only persistent source view independent of runtime lowering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DocumentSourceRef<'a> {
    pub id: DocumentSourceId,
    pub owner: DocumentSourceOwner,
    pub label: &'a str,
    pub suppressed: bool,
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

/// Persistent identities created by one point-defined curve mirror construction.
#[derive(Clone, Debug, PartialEq)]
pub struct MirroredCurveIds {
    pub source_curve: CurveId,
    pub mirrored_curve: CurveId,
    pub point_pairs: Vec<(DesignPointId, DesignPointId)>,
    pub symmetry_constraints: Vec<DocumentConstraintId>,
}

/// Persistent identities changed by one coordinated mirrored B-spline refinement.
#[derive(Clone, Debug, PartialEq)]
pub struct DocumentMirroredBSplineInsertion {
    pub source: DocumentBSplineInsertion,
    pub mirrored: DocumentBSplineInsertion,
    pub symmetry_constraint: DocumentConstraintId,
}

/// Validated input for one atomic associative line-line fillet construction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineLineFilletRequest {
    pub first: CurveSpan,
    pub first_side: DocumentCurveNormalSide,
    pub second: CurveSpan,
    pub second_side: DocumentCurveNormalSide,
    pub endpoint_order: DocumentFilletEndpointOrder,
    pub sweep: DocumentArcSweep,
    pub radius: f64,
    pub radius_mode: DocumentDimensionMode,
}

/// Persistent identities created by one associative line-line fillet construction.
#[derive(Clone, Debug, PartialEq)]
pub struct LineLineFilletIds {
    pub constraint: DocumentConstraintId,
    pub arc: CurveId,
    pub center: DesignPointId,
    pub radius: DesignScalarId,
    pub start_angle: DesignScalarId,
    pub end_angle: DesignScalarId,
    pub contacts: [ContactId; 2],
    pub contact_parameters: [DesignScalarId; 2],
    pub radius_dimension: DocumentDimensionId,
    pub radius_target: DesignScalarId,
}

/// One generic fillet parent request with explicit root and visible-endpoint state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveFilletParentRequest {
    pub curve: CurveSpan,
    pub parameter: f64,
    pub winding: i32,
    pub neighborhood: ContactNeighborhood,
    pub side: DocumentCurveNormalSide,
    pub trim_endpoint: DocumentFilletTrimEndpoint,
    pub periodic_anchor: Option<DocumentTrimParameter>,
}

/// Validated input for one atomic associative generic curve fillet construction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveCurveFilletRequest {
    pub first: CurveFilletParentRequest,
    pub second: CurveFilletParentRequest,
    pub endpoint_order: DocumentFilletEndpointOrder,
    pub sweep: DocumentArcSweep,
    pub radius: f64,
    pub radius_mode: DocumentDimensionMode,
}

/// Persistent identities created by one associative generic curve fillet construction.
pub type CurveCurveFilletIds = LineLineFilletIds;

/// Field-opaque, checkpoint-serializable never-reuse cursors for one persistent sketch namespace.
///
/// Frozen sketch v1-v4 records the cursors of its current graph, but a historical
/// graph cannot itself retain identities consumed by a later abandoned branch.
/// Hosts retain this lifecycle maximum beside checkpoints and merge it back into
/// a restored document before permitting further authoring.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SketchPersistentIdentityHighWater {
    document: DocumentId,
    next_id: PersistentId,
    spline_span_cursors: BTreeMap<CurveId, u32>,
}

impl SketchPersistentIdentityHighWater {
    /// Persistent sketch namespace owned by these allocator cursors.
    #[must_use]
    pub const fn document(&self) -> DocumentId {
        self.document
    }

    /// Returns the componentwise maximum of two cursors for the same sketch.
    ///
    /// # Errors
    ///
    /// Rejects cursors from different persistent sketch namespaces.
    pub fn merged(&self, other: &Self) -> Result<Self, DocumentError> {
        if self.document != other.document {
            return invalid(
                "persistent identity high-water",
                "cannot merge cursors from different sketch documents",
            );
        }
        self.validate()?;
        other.validate()?;
        let additional = other
            .spline_span_cursors
            .keys()
            .filter(|curve| !self.spline_span_cursors.contains_key(curve))
            .count();
        let merged_count = self.spline_span_cursors.len().saturating_add(additional);
        validate_persistent_spline_cursor_count(merged_count)?;
        let mut merged = self.clone();
        merged.next_id = merged.next_id.max(other.next_id);
        for (curve, cursor) in &other.spline_span_cursors {
            merged
                .spline_span_cursors
                .entry(*curve)
                .and_modify(|retained| *retained = (*retained).max(*cursor))
                .or_insert(*cursor);
        }
        Ok(merged)
    }

    fn validate(&self) -> Result<(), DocumentError> {
        validate_persistent_spline_cursor_count(self.spline_span_cursors.len())?;
        let document = self.document.0.as_u128();
        let next_id = self.next_id.as_u128();
        if document == 0 || next_id <= document {
            return invalid(
                "persistent identity high-water",
                "next object cursor must be greater than the nonzero document identity",
            );
        }
        for (curve, cursor) in &self.spline_span_cursors {
            let curve = curve.0.as_u128();
            if curve <= document || curve >= next_id {
                return invalid(
                    "persistent identity high-water",
                    "spline cursor curve must be an allocated object identity in this document",
                );
            }
            if *cursor == 0 {
                return invalid(
                    "persistent identity high-water",
                    "spline next-span cursor must be nonzero",
                );
            }
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SketchPersistentIdentityHighWaterWire {
    document: DocumentId,
    next_id: PersistentId,
    #[serde(deserialize_with = "deserialize_spline_span_cursors")]
    spline_span_cursors: BTreeMap<CurveId, u32>,
}

impl<'de> Deserialize<'de> for SketchPersistentIdentityHighWater {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = SketchPersistentIdentityHighWaterWire::deserialize(deserializer)?;
        let high_water = Self {
            document: wire.document,
            next_id: wire.next_id,
            spline_span_cursors: wire.spline_span_cursors,
        };
        high_water.validate().map_err(serde::de::Error::custom)?;
        Ok(high_water)
    }
}

fn validate_persistent_spline_cursor_count(count: usize) -> Result<(), DocumentError> {
    if count > MAX_PERSISTENT_SPLINE_SPAN_CURSORS {
        return Err(DocumentError::ResourceLimit {
            resource: "persistent spline span allocator cursors",
            actual: count,
            limit: MAX_PERSISTENT_SPLINE_SPAN_CURSORS,
        });
    }
    Ok(())
}

fn deserialize_spline_span_cursors<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<CurveId, u32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct BoundedCursorMap;

    impl<'de> serde::de::Visitor<'de> for BoundedCursorMap {
        type Value = BTreeMap<CurveId, u32>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                formatter,
                "at most {MAX_PERSISTENT_SPLINE_SPAN_CURSORS} unique spline cursor entries"
            )
        }

        fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::MapAccess<'de>,
        {
            if access
                .size_hint()
                .is_some_and(|size| size > MAX_PERSISTENT_SPLINE_SPAN_CURSORS)
            {
                return Err(serde::de::Error::custom(
                    "persistent spline span allocator cursor limit exceeded",
                ));
            }
            let mut cursors = BTreeMap::new();
            while let Some((curve, cursor)) = access.next_entry()? {
                if !cursors.contains_key(&curve)
                    && cursors.len() == MAX_PERSISTENT_SPLINE_SPAN_CURSORS
                {
                    return Err(serde::de::Error::custom(
                        "persistent spline span allocator cursor limit exceeded",
                    ));
                }
                if cursors.insert(curve, cursor).is_some() {
                    return Err(serde::de::Error::custom(
                        "duplicate persistent spline span allocator cursor",
                    ));
                }
            }
            Ok(cursors)
        }
    }

    deserializer.deserialize_map(BoundedCursorMap)
}

#[cfg(test)]
mod persistent_identity_high_water_tests {
    use super::*;

    #[test]
    fn oversized_spline_cursor_history_rejects_merge_and_streaming_decode() {
        let document = SketchDocument::new(1.0).expect("document");
        let retained = document.persistent_identity_high_water();
        let mut oversized = retained.clone();
        for ordinal in 0..=MAX_PERSISTENT_SPLINE_SPAN_CURSORS {
            oversized
                .spline_span_cursors
                .insert(CurveId(PersistentId::from_u128(ordinal as u128 + 1)), 1);
        }

        assert!(matches!(
            retained.merged(&oversized),
            Err(DocumentError::ResourceLimit {
                resource: "persistent spline span allocator cursors",
                actual,
                limit: MAX_PERSISTENT_SPLINE_SPAN_CURSORS,
            }) if actual == MAX_PERSISTENT_SPLINE_SPAN_CURSORS + 1
        ));

        let mut unchanged = document.clone();
        let before = unchanged.clone();
        assert!(matches!(
            unchanged.retain_persistent_identity_high_water(&oversized),
            Err(DocumentError::ResourceLimit {
                resource: "persistent spline span allocator cursors",
                actual,
                limit: MAX_PERSISTENT_SPLINE_SPAN_CURSORS,
            }) if actual == MAX_PERSISTENT_SPLINE_SPAN_CURSORS + 1
        ));
        assert_eq!(unchanged, before);

        let encoded = serde_json::to_string(&oversized).expect("oversized test encoding");
        assert!(
            serde_json::from_str::<SketchPersistentIdentityHighWater>(&encoded).is_err(),
            "streaming decode must stop before admitting an unbounded cursor map"
        );
    }

    #[test]
    fn duplicate_spline_cursor_keys_reject_during_streaming_decode() {
        let document = SketchDocument::new(1.0).expect("document");
        let high_water = document.persistent_identity_high_water();
        let curve = PersistentId::from_u128(17).to_string();
        let encoded = format!(
            r#"{{"document":"{}","next_id":"{}","spline_span_cursors":{{"{curve}":4,"{curve}":5}}}}"#,
            high_water.document.0, high_water.next_id
        );

        assert!(serde_json::from_str::<SketchPersistentIdentityHighWater>(&encoded).is_err());
    }

    #[test]
    fn malformed_cursor_relationships_reject_decode_merge_and_retain() {
        let mut document =
            SketchDocument::with_id(1.0, DocumentId(PersistentId::from_u128(1))).expect("document");
        let allocated = document.add_point("allocated", [0.0, 0.0]).expect("point");
        let valid = document.persistent_identity_high_water();

        let mut nonadvancing = valid.clone();
        nonadvancing.next_id = nonadvancing.document.0;

        let mut future_curve = valid.clone();
        future_curve
            .spline_span_cursors
            .insert(CurveId(future_curve.next_id), 1);

        let mut zero_span_cursor = valid.clone();
        zero_span_cursor
            .spline_span_cursors
            .insert(CurveId(allocated.0), 0);

        for malformed in [nonadvancing, future_curve, zero_span_cursor] {
            let encoded = serde_json::to_string(&malformed).expect("malformed encoding");
            assert!(
                serde_json::from_str::<SketchPersistentIdentityHighWater>(&encoded).is_err(),
                "semantic cursor invariants must be checked after field decoding"
            );
            assert!(matches!(
                valid.merged(&malformed),
                Err(DocumentError::InvalidField {
                    field: "persistent identity high-water",
                    ..
                })
            ));
            let mut unchanged = document.clone();
            assert!(matches!(
                unchanged.retain_persistent_identity_high_water(&malformed),
                Err(DocumentError::InvalidField {
                    field: "persistent identity high-water",
                    ..
                })
            ));
            assert_eq!(unchanged, document);
        }
    }

    #[test]
    fn exhausted_object_cursor_rejects_allocation_without_mutation() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let mut high_water = document.persistent_identity_high_water();
        high_water.next_id = PersistentId::from_u128(u128::MAX);
        document
            .retain_persistent_identity_high_water(&high_water)
            .expect("retain exhausted cursor");
        let before = document.clone();

        assert!(matches!(
            document.add_point("must not allocate", [0.0, 0.0]),
            Err(DocumentError::IdExhausted)
        ));
        assert_eq!(document, before);
    }

    #[test]
    fn exhausted_spline_cursor_rejects_refinement_without_topology_mutation() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let controls = [[0.0, 0.0], [1.0, 2.0], [2.0, -1.0], [3.0, 1.5], [4.0, 0.0]]
            .map(|position| document.add_point("control", position).expect("control"))
            .to_vec();
        let curve = document
            .add_curve(
                "clamped cubic",
                CurveDefinition::BSpline {
                    form: DocumentBSplineForm::Clamped,
                    degree: 3,
                    controls,
                    knots: vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0],
                    span_ids: vec![41, 73],
                    next_span_id: 100,
                },
            )
            .expect("B-spline");
        let mut high_water = document.persistent_identity_high_water();
        high_water.spline_span_cursors.insert(curve, u32::MAX);
        document
            .retain_persistent_identity_high_water(&high_water)
            .expect("retain exhausted span cursor");
        let before = document.clone();

        assert!(matches!(
            document.insert_bspline_knot(curve, 0.25),
            Err(DocumentError::IdExhausted)
        ));
        assert_eq!(document, before);
    }

    #[test]
    fn foreign_high_water_rejects_without_mutation() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let foreign = SketchDocument::new(1.0)
            .expect("foreign document")
            .persistent_identity_high_water();
        let before = document.clone();

        assert!(matches!(
            document.retain_persistent_identity_high_water(&foreign),
            Err(DocumentError::InvalidField {
                field: "persistent identity high-water",
                ..
            })
        ));
        assert_eq!(document, before);
    }
}

/// Versioned persistent sketch graph. Runtime solver IDs never appear here.
#[derive(Clone, Debug, PartialEq)]
pub struct SketchDocument {
    version: u32,
    id: DocumentId,
    next_id: PersistentId,
    model_scale: f64,
    points: Vec<DesignPoint>,
    scalars: Vec<DesignScalar>,
    curves: Vec<DesignCurve>,
    contacts: Vec<ContactSlot>,
    trim_views: Vec<DocumentCurveTrimView>,
    constraints: Vec<DocumentConstraint>,
    dimensions: Vec<DocumentDimension>,
    parameters: Vec<DocumentParameter>,
    parameter_bindings: Vec<DocumentParameterBinding>,
    parameter_outputs: Vec<DocumentParameterOutput>,
    external_bindings: Vec<DocumentExternalBinding>,
    source_order: Vec<DocumentSourceId>,
    geometry_roles: BTreeMap<CurveId, GeometryRole>,
    user_inactive_elements: BTreeSet<DocumentElementId>,
    host_activation: Option<HostConfigurationActivation>,
    /// Ownership for semantic catalogs persisted outside frozen sketch v1-v4.
    semantic_source_reservations: BTreeMap<DocumentSourceId, DocumentSourceId>,
    mutation_validation_deferred: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DocumentConstraintV2 {
    id: DocumentConstraintId,
    source_id: DocumentSourceId,
    label: String,
    suppressed: bool,
    definition: DocumentConstraintDefinitionV2,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum DocumentConstraintDefinitionV2 {
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

impl From<DocumentConstraintV2> for DocumentConstraint {
    fn from(constraint: DocumentConstraintV2) -> Self {
        Self {
            id: constraint.id,
            source_id: constraint.source_id,
            label: constraint.label,
            suppressed: constraint.suppressed,
            definition: constraint.definition.into(),
        }
    }
}

impl From<DocumentConstraintDefinitionV2> for DocumentConstraintDefinition {
    #[allow(clippy::too_many_lines)]
    fn from(definition: DocumentConstraintDefinitionV2) -> Self {
        use DocumentConstraintDefinitionV2 as V;
        match definition {
            V::FixedPoint { point, target } => Self::FixedPoint { point, target },
            V::FixedCoordinate {
                point,
                axis,
                target,
            } => Self::FixedCoordinate {
                point,
                axis,
                target,
            },
            V::Coincident { first, second } => Self::Coincident { first, second },
            V::Horizontal { line } => Self::Horizontal { line },
            V::Vertical { line } => Self::Vertical { line },
            V::PointOnCurve { point, contact } => Self::PointOnCurve { point, contact },
            V::Parallel { first, second } => Self::Parallel { first, second },
            V::Perpendicular { first, second } => Self::Perpendicular { first, second },
            V::EqualLength { first, second } => Self::EqualLength { first, second },
            V::EqualRadius { first, second } => Self::EqualRadius { first, second },
            V::Midpoint { point, line } => Self::Midpoint { point, line },
            V::SymmetricAboutLine {
                first,
                second,
                line,
            } => Self::SymmetricAboutLine {
                first,
                second,
                line,
            },
            V::LineCircleTangency {
                line_contact,
                circle_contact,
                side,
            } => Self::LineCircleTangency {
                line_contact,
                circle_contact,
                side,
            },
            V::CircleCircleTangency {
                first,
                second,
                mode,
                center_direction,
            } => Self::CircleCircleTangency {
                first,
                second,
                mode,
                center_direction,
            },
            V::CircleArcTangency {
                circle_contact,
                arc_contact,
                side,
            } => Self::CircleArcTangency {
                circle_contact,
                arc_contact,
                side,
            },
            V::LineCurveTangency {
                line,
                endpoint,
                curve_contact,
            } => Self::LineCurveTangency {
                line,
                endpoint,
                curve_contact,
            },
            V::CurveCurveContact {
                first_contact,
                second_contact,
            } => Self::CurveCurveContact {
                first_contact,
                second_contact,
            },
            V::CurveCurveTangency {
                first_contact,
                second_contact,
            } => Self::CurveCurveTangency {
                first_contact,
                second_contact,
            },
            V::CurveDirection {
                line,
                curve_contact,
                relation,
            } => Self::CurveDirection {
                line,
                curve_contact,
                relation,
            },
            V::EqualCurvature {
                first_contact,
                second_contact,
                relation,
            } => Self::EqualCurvature {
                first_contact,
                second_contact,
                relation,
            },
            V::EndpointContinuity {
                first_contact,
                second_contact,
                continuity,
            } => Self::EndpointContinuity {
                first_contact,
                second_contact,
                continuity,
            },
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DocumentConstraintV3 {
    id: DocumentConstraintId,
    source_id: DocumentSourceId,
    label: String,
    suppressed: bool,
    definition: DocumentConstraintDefinitionV3,
}

/// Frozen sketch-v4 constraint wire record. Keep this exhaustive over the language
/// accepted before M71 rather than serializing the evolving in-memory enum directly.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DocumentConstraintV4 {
    id: DocumentConstraintId,
    source_id: DocumentSourceId,
    label: String,
    suppressed: bool,
    definition: DocumentConstraintDefinitionV4,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum DocumentConstraintDefinitionV4 {
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
    ExternalPointCoincident {
        point: DesignPointId,
        external: DocumentExternalPointRef,
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
    ExternalLineCollinear {
        line: DocumentLineSupportRef,
        external: DocumentExternalLineSupportRef,
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
    LineLineFillet {
        arc: CurveId,
        first_contact: ContactId,
        first_side: DocumentCurveNormalSide,
        second_contact: ContactId,
        second_side: DocumentCurveNormalSide,
        endpoint_order: DocumentFilletEndpointOrder,
    },
    CurveCurveFillet {
        arc: CurveId,
        first_contact: ContactId,
        first_side: DocumentCurveNormalSide,
        first_trim_endpoint: DocumentFilletTrimEndpoint,
        second_contact: ContactId,
        second_side: DocumentCurveNormalSide,
        second_trim_endpoint: DocumentFilletTrimEndpoint,
        endpoint_order: DocumentFilletEndpointOrder,
    },
}

impl From<DocumentConstraintV4> for DocumentConstraint {
    fn from(value: DocumentConstraintV4) -> Self {
        Self {
            id: value.id,
            source_id: value.source_id,
            label: value.label,
            suppressed: value.suppressed,
            definition: value.definition.into(),
        }
    }
}

impl From<DocumentConstraintDefinitionV4> for DocumentConstraintDefinition {
    #[allow(clippy::too_many_lines)]
    fn from(value: DocumentConstraintDefinitionV4) -> Self {
        use DocumentConstraintDefinitionV4 as V;
        match value {
            V::FixedPoint { point, target } => Self::FixedPoint { point, target },
            V::FixedCoordinate {
                point,
                axis,
                target,
            } => Self::FixedCoordinate {
                point,
                axis,
                target,
            },
            V::Coincident { first, second } => Self::Coincident { first, second },
            V::ExternalPointCoincident { point, external } => {
                Self::ExternalPointCoincident { point, external }
            }
            V::Horizontal { line } => Self::Horizontal { line },
            V::Vertical { line } => Self::Vertical { line },
            V::PointOnCurve { point, contact } => Self::PointOnCurve { point, contact },
            V::Parallel { first, second } => Self::Parallel { first, second },
            V::Perpendicular { first, second } => Self::Perpendicular { first, second },
            V::ExternalLineCollinear { line, external } => {
                Self::ExternalLineCollinear { line, external }
            }
            V::EqualLength { first, second } => Self::EqualLength { first, second },
            V::EqualRadius { first, second } => Self::EqualRadius { first, second },
            V::Midpoint { point, line } => Self::Midpoint { point, line },
            V::SymmetricAboutLine {
                first,
                second,
                line,
            } => Self::SymmetricAboutLine {
                first,
                second,
                line,
            },
            V::LineCircleTangency {
                line_contact,
                circle_contact,
                side,
            } => Self::LineCircleTangency {
                line_contact,
                circle_contact,
                side,
            },
            V::CircleCircleTangency {
                first,
                second,
                mode,
                center_direction,
            } => Self::CircleCircleTangency {
                first,
                second,
                mode,
                center_direction,
            },
            V::CircleArcTangency {
                circle_contact,
                arc_contact,
                side,
            } => Self::CircleArcTangency {
                circle_contact,
                arc_contact,
                side,
            },
            V::LineCurveTangency {
                line,
                endpoint,
                curve_contact,
            } => Self::LineCurveTangency {
                line,
                endpoint,
                curve_contact,
            },
            V::CurveCurveContact {
                first_contact,
                second_contact,
            } => Self::CurveCurveContact {
                first_contact,
                second_contact,
            },
            V::CurveCurveTangency {
                first_contact,
                second_contact,
            } => Self::CurveCurveTangency {
                first_contact,
                second_contact,
            },
            V::CurveDirection {
                line,
                curve_contact,
                relation,
            } => Self::CurveDirection {
                line,
                curve_contact,
                relation,
            },
            V::EqualCurvature {
                first_contact,
                second_contact,
                relation,
            } => Self::EqualCurvature {
                first_contact,
                second_contact,
                relation,
            },
            V::EndpointContinuity {
                first_contact,
                second_contact,
                continuity,
            } => Self::EndpointContinuity {
                first_contact,
                second_contact,
                continuity,
            },
            V::LineLineFillet {
                arc,
                first_contact,
                first_side,
                second_contact,
                second_side,
                endpoint_order,
            } => Self::LineLineFillet {
                arc,
                first_contact,
                first_side,
                second_contact,
                second_side,
                endpoint_order,
            },
            V::CurveCurveFillet {
                arc,
                first_contact,
                first_side,
                first_trim_endpoint,
                second_contact,
                second_side,
                second_trim_endpoint,
                endpoint_order,
            } => Self::CurveCurveFillet {
                arc,
                first_contact,
                first_side,
                first_trim_endpoint,
                second_contact,
                second_side,
                second_trim_endpoint,
                endpoint_order,
            },
        }
    }
}

impl TryFrom<&DocumentConstraint> for DocumentConstraintV4 {
    type Error = DocumentError;

    fn try_from(value: &DocumentConstraint) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.id,
            source_id: value.source_id,
            label: value.label.clone(),
            suppressed: value.suppressed,
            definition: DocumentConstraintDefinitionV4::try_from(&value.definition)?,
        })
    }
}

impl TryFrom<&DocumentConstraintDefinition> for DocumentConstraintDefinitionV4 {
    type Error = DocumentError;

    #[allow(clippy::too_many_lines)]
    fn try_from(value: &DocumentConstraintDefinition) -> Result<Self, Self::Error> {
        use DocumentConstraintDefinition as C;
        Ok(match value {
            C::FixedPoint { point, target } => Self::FixedPoint {
                point: *point,
                target: *target,
            },
            C::FixedCoordinate {
                point,
                axis,
                target,
            } => Self::FixedCoordinate {
                point: *point,
                axis: *axis,
                target: *target,
            },
            C::Coincident { first, second } => Self::Coincident {
                first: *first,
                second: *second,
            },
            C::ExternalPointCoincident { point, external } => Self::ExternalPointCoincident {
                point: *point,
                external: *external,
            },
            C::Horizontal { line } => Self::Horizontal { line: *line },
            C::Vertical { line } => Self::Vertical { line: *line },
            C::PointOnCurve { point, contact } => Self::PointOnCurve {
                point: *point,
                contact: *contact,
            },
            C::Parallel { first, second } => Self::Parallel {
                first: *first,
                second: *second,
            },
            C::Perpendicular { first, second } => Self::Perpendicular {
                first: *first,
                second: *second,
            },
            C::ExternalLineCollinear { line, external } => Self::ExternalLineCollinear {
                line: *line,
                external: *external,
            },
            C::EqualLength { first, second } => Self::EqualLength {
                first: *first,
                second: *second,
            },
            C::EqualRadius { first, second } => Self::EqualRadius {
                first: *first,
                second: *second,
            },
            C::Midpoint { point, line } => Self::Midpoint {
                point: *point,
                line: *line,
            },
            C::SymmetricAboutLine {
                first,
                second,
                line,
            } => Self::SymmetricAboutLine {
                first: *first,
                second: *second,
                line: *line,
            },
            C::LineCircleTangency {
                line_contact,
                circle_contact,
                side,
            } => Self::LineCircleTangency {
                line_contact: *line_contact,
                circle_contact: *circle_contact,
                side: *side,
            },
            C::CircleCircleTangency {
                first,
                second,
                mode,
                center_direction,
            } => Self::CircleCircleTangency {
                first: *first,
                second: *second,
                mode: *mode,
                center_direction: *center_direction,
            },
            C::CircleArcTangency {
                circle_contact,
                arc_contact,
                side,
            } => Self::CircleArcTangency {
                circle_contact: *circle_contact,
                arc_contact: *arc_contact,
                side: *side,
            },
            C::LineCurveTangency {
                line,
                endpoint,
                curve_contact,
            } => Self::LineCurveTangency {
                line: *line,
                endpoint: *endpoint,
                curve_contact: *curve_contact,
            },
            C::CurveCurveContact {
                first_contact,
                second_contact,
            } => Self::CurveCurveContact {
                first_contact: *first_contact,
                second_contact: *second_contact,
            },
            C::CurveCurveTangency {
                first_contact,
                second_contact,
            } => Self::CurveCurveTangency {
                first_contact: *first_contact,
                second_contact: *second_contact,
            },
            C::CurveDirection {
                line,
                curve_contact,
                relation,
            } => Self::CurveDirection {
                line: *line,
                curve_contact: *curve_contact,
                relation: *relation,
            },
            C::EqualCurvature {
                first_contact,
                second_contact,
                relation,
            } => Self::EqualCurvature {
                first_contact: *first_contact,
                second_contact: *second_contact,
                relation: *relation,
            },
            C::EndpointContinuity {
                first_contact,
                second_contact,
                continuity,
            } => Self::EndpointContinuity {
                first_contact: *first_contact,
                second_contact: *second_contact,
                continuity: *continuity,
            },
            C::LineLineFillet {
                arc,
                first_contact,
                first_side,
                second_contact,
                second_side,
                endpoint_order,
            } => Self::LineLineFillet {
                arc: *arc,
                first_contact: *first_contact,
                first_side: *first_side,
                second_contact: *second_contact,
                second_side: *second_side,
                endpoint_order: *endpoint_order,
            },
            C::CurveCurveFillet {
                arc,
                first_contact,
                first_side,
                first_trim_endpoint,
                second_contact,
                second_side,
                second_trim_endpoint,
                endpoint_order,
            } => Self::CurveCurveFillet {
                arc: *arc,
                first_contact: *first_contact,
                first_side: *first_side,
                first_trim_endpoint: *first_trim_endpoint,
                second_contact: *second_contact,
                second_side: *second_side,
                second_trim_endpoint: *second_trim_endpoint,
                endpoint_order: *endpoint_order,
            },
            C::HorizontalPoints { .. }
            | C::VerticalPoints { .. }
            | C::HorizontalPointToMidpoint { .. }
            | C::VerticalPointToMidpoint { .. }
            | C::Concentric { .. }
            | C::Collinear { .. } => return Err(DocumentError::UnsupportedM71State),
            C::CoincidentWithOrigin { .. }
            | C::PointOnDatumAxis { .. }
            | C::CollinearWithDatumAxis { .. }
            | C::SymmetricAboutDatumAxis { .. } => {
                return Err(DocumentError::UnsupportedM74State);
            }
        })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(untagged)]
enum DocumentConstraintDefinitionV3 {
    Previous(DocumentConstraintDefinitionV2),
    LineFillet(DocumentLineFilletDefinitionV3),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum DocumentLineFilletDefinitionV3 {
    LineLineFillet {
        arc: CurveId,
        first_contact: ContactId,
        first_side: DocumentCurveNormalSide,
        second_contact: ContactId,
        second_side: DocumentCurveNormalSide,
        endpoint_order: DocumentFilletEndpointOrder,
    },
}

impl From<DocumentConstraintV3> for DocumentConstraint {
    fn from(constraint: DocumentConstraintV3) -> Self {
        Self {
            id: constraint.id,
            source_id: constraint.source_id,
            label: constraint.label,
            suppressed: constraint.suppressed,
            definition: constraint.definition.into(),
        }
    }
}

impl From<DocumentConstraintDefinitionV3> for DocumentConstraintDefinition {
    fn from(definition: DocumentConstraintDefinitionV3) -> Self {
        match definition {
            DocumentConstraintDefinitionV3::Previous(previous) => previous.into(),
            DocumentConstraintDefinitionV3::LineFillet(
                DocumentLineFilletDefinitionV3::LineLineFillet {
                    arc,
                    first_contact,
                    first_side,
                    second_contact,
                    second_side,
                    endpoint_order,
                },
            ) => Self::LineLineFillet {
                arc,
                first_contact,
                first_side,
                second_contact,
                second_side,
                endpoint_order,
            },
        }
    }
}

/// Frozen version-one wire representation. The in-memory document is deliberately
/// separate so future versions migrate explicitly instead of mutating this schema.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SketchDocumentV1 {
    version: u32,
    id: DocumentId,
    next_id: PersistentId,
    model_scale: f64,
    points: Vec<DesignPoint>,
    scalars: Vec<DesignScalar>,
    curves: Vec<DesignCurve>,
    contacts: Vec<ContactSlot>,
    constraints: Vec<DocumentConstraintV2>,
    dimensions: Vec<DocumentDimensionV1>,
    source_order: Vec<DocumentSourceId>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DocumentDimensionV1 {
    id: DocumentDimensionId,
    source_id: DocumentSourceId,
    label: String,
    mode: DocumentDimensionMode,
    suppressed: bool,
    definition: DocumentDimensionDefinitionV1,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum DocumentDimensionDefinitionV1 {
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

impl From<SketchDocumentV1> for SketchDocument {
    fn from(document: SketchDocumentV1) -> Self {
        Self {
            version: SKETCH_DOCUMENT_VERSION,
            id: document.id,
            next_id: document.next_id,
            model_scale: document.model_scale,
            points: document.points,
            scalars: document.scalars,
            curves: document.curves,
            contacts: document.contacts,
            trim_views: Vec::new(),
            constraints: document
                .constraints
                .into_iter()
                .map(DocumentConstraint::from)
                .collect(),
            dimensions: document
                .dimensions
                .into_iter()
                .map(DocumentDimension::from)
                .collect(),
            parameters: Vec::new(),
            parameter_bindings: Vec::new(),
            parameter_outputs: Vec::new(),
            external_bindings: Vec::new(),
            source_order: document.source_order,
            geometry_roles: BTreeMap::new(),
            user_inactive_elements: BTreeSet::new(),
            host_activation: None,
            semantic_source_reservations: BTreeMap::new(),
            mutation_validation_deferred: false,
        }
    }
}

impl From<DocumentDimensionV1> for DocumentDimension {
    fn from(dimension: DocumentDimensionV1) -> Self {
        Self {
            id: dimension.id,
            source_id: dimension.source_id,
            label: dimension.label,
            mode: dimension.mode,
            suppressed: dimension.suppressed,
            definition: dimension.definition.into(),
        }
    }
}

impl From<DocumentDimensionDefinitionV1> for DocumentDimensionDefinition {
    fn from(definition: DocumentDimensionDefinitionV1) -> Self {
        match definition {
            DocumentDimensionDefinitionV1::PointDistance {
                first,
                second,
                target,
            } => Self::PointDistance {
                first,
                second,
                target,
            },
            DocumentDimensionDefinitionV1::CurveLength { curve, target } => {
                Self::CurveLength { curve, target }
            }
            DocumentDimensionDefinitionV1::Radius { curve, target } => {
                Self::Radius { curve, target }
            }
            DocumentDimensionDefinitionV1::Diameter { curve, target } => {
                Self::Diameter { curve, target }
            }
            DocumentDimensionDefinitionV1::OrientedAngle {
                first,
                second,
                target,
                orientation,
            } => Self::OrientedAngle {
                first,
                second,
                target,
                orientation,
            },
        }
    }
}

/// Frozen v2-v4 dimension wire record. The private draft-v5 side table owns
/// post-v4 profile-offset dimensions so historical canonical bytes cannot widen.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DocumentDimensionV4 {
    id: DocumentDimensionId,
    source_id: DocumentSourceId,
    label: String,
    mode: DocumentDimensionMode,
    suppressed: bool,
    definition: DocumentDimensionDefinitionV4,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum DocumentDimensionDefinitionV4 {
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
    SupportingLineOffset {
        source: CurveSpan,
        target_segment: CurveSpan,
        target: DesignScalarId,
        side: DocumentLineSide,
        orientation: DocumentLineOffsetOrientation,
    },
    ExactTranslatedSegmentOffset {
        source: CurveSpan,
        target_segment: CurveSpan,
        target: DesignScalarId,
        side: DocumentLineSide,
        orientation: DocumentLineOffsetOrientation,
    },
}

impl TryFrom<&DocumentDimension> for DocumentDimensionV4 {
    type Error = DocumentError;

    fn try_from(value: &DocumentDimension) -> Result<Self, Self::Error> {
        use DocumentDimensionDefinition as D;
        let definition = match &value.definition {
            D::PointDistance {
                first,
                second,
                target,
            } => DocumentDimensionDefinitionV4::PointDistance {
                first: *first,
                second: *second,
                target: *target,
            },
            D::CurveLength { curve, target } => DocumentDimensionDefinitionV4::CurveLength {
                curve: *curve,
                target: *target,
            },
            D::Radius { curve, target } => DocumentDimensionDefinitionV4::Radius {
                curve: *curve,
                target: *target,
            },
            D::Diameter { curve, target } => DocumentDimensionDefinitionV4::Diameter {
                curve: *curve,
                target: *target,
            },
            D::OrientedAngle {
                first,
                second,
                target,
                orientation,
            } => DocumentDimensionDefinitionV4::OrientedAngle {
                first: *first,
                second: *second,
                target: *target,
                orientation: *orientation,
            },
            D::SupportingLineOffset {
                source,
                target_segment,
                target,
                side,
                orientation,
            } => DocumentDimensionDefinitionV4::SupportingLineOffset {
                source: *source,
                target_segment: *target_segment,
                target: *target,
                side: *side,
                orientation: *orientation,
            },
            D::ExactTranslatedSegmentOffset {
                source,
                target_segment,
                target,
                side,
                orientation,
            } => DocumentDimensionDefinitionV4::ExactTranslatedSegmentOffset {
                source: *source,
                target_segment: *target_segment,
                target: *target,
                side: *side,
                orientation: *orientation,
            },
            D::ProfileOffset { .. } => return Err(DocumentError::UnsupportedM80State),
        };
        Ok(Self {
            id: value.id,
            source_id: value.source_id,
            label: value.label.clone(),
            mode: value.mode,
            suppressed: value.suppressed,
            definition,
        })
    }
}

impl From<DocumentDimensionV4> for DocumentDimension {
    fn from(value: DocumentDimensionV4) -> Self {
        use DocumentDimensionDefinitionV4 as D;
        let definition = match value.definition {
            D::PointDistance {
                first,
                second,
                target,
            } => DocumentDimensionDefinition::PointDistance {
                first,
                second,
                target,
            },
            D::CurveLength { curve, target } => {
                DocumentDimensionDefinition::CurveLength { curve, target }
            }
            D::Radius { curve, target } => DocumentDimensionDefinition::Radius { curve, target },
            D::Diameter { curve, target } => {
                DocumentDimensionDefinition::Diameter { curve, target }
            }
            D::OrientedAngle {
                first,
                second,
                target,
                orientation,
            } => DocumentDimensionDefinition::OrientedAngle {
                first,
                second,
                target,
                orientation,
            },
            D::SupportingLineOffset {
                source,
                target_segment,
                target,
                side,
                orientation,
            } => DocumentDimensionDefinition::SupportingLineOffset {
                source,
                target_segment,
                target,
                side,
                orientation,
            },
            D::ExactTranslatedSegmentOffset {
                source,
                target_segment,
                target,
                side,
                orientation,
            } => DocumentDimensionDefinition::ExactTranslatedSegmentOffset {
                source,
                target_segment,
                target,
                side,
                orientation,
            },
        };
        Self {
            id: value.id,
            source_id: value.source_id,
            label: value.label,
            mode: value.mode,
            suppressed: value.suppressed,
            definition,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SketchDocumentV2 {
    version: u32,
    id: DocumentId,
    next_id: PersistentId,
    model_scale: f64,
    points: Vec<DesignPoint>,
    scalars: Vec<DesignScalar>,
    curves: Vec<DesignCurve>,
    contacts: Vec<ContactSlot>,
    constraints: Vec<DocumentConstraintV2>,
    dimensions: Vec<DocumentDimensionV4>,
    source_order: Vec<DocumentSourceId>,
}

impl From<SketchDocumentV2> for SketchDocument {
    fn from(document: SketchDocumentV2) -> Self {
        Self {
            version: SKETCH_DOCUMENT_VERSION,
            id: document.id,
            next_id: document.next_id,
            model_scale: document.model_scale,
            points: document.points,
            scalars: document.scalars,
            curves: document.curves,
            contacts: document.contacts,
            trim_views: Vec::new(),
            constraints: document
                .constraints
                .into_iter()
                .map(DocumentConstraint::from)
                .collect(),
            dimensions: document
                .dimensions
                .into_iter()
                .map(DocumentDimension::from)
                .collect(),
            parameters: Vec::new(),
            parameter_bindings: Vec::new(),
            parameter_outputs: Vec::new(),
            external_bindings: Vec::new(),
            source_order: document.source_order,
            geometry_roles: BTreeMap::new(),
            user_inactive_elements: BTreeSet::new(),
            host_activation: None,
            semantic_source_reservations: BTreeMap::new(),
            mutation_validation_deferred: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SketchDocumentV3 {
    version: u32,
    id: DocumentId,
    next_id: PersistentId,
    model_scale: f64,
    points: Vec<DesignPoint>,
    scalars: Vec<DesignScalar>,
    curves: Vec<DesignCurve>,
    contacts: Vec<ContactSlot>,
    constraints: Vec<DocumentConstraintV3>,
    dimensions: Vec<DocumentDimensionV4>,
    source_order: Vec<DocumentSourceId>,
}

impl From<SketchDocumentV3> for SketchDocument {
    fn from(document: SketchDocumentV3) -> Self {
        Self {
            version: SKETCH_DOCUMENT_VERSION,
            id: document.id,
            next_id: document.next_id,
            model_scale: document.model_scale,
            points: document.points,
            scalars: document.scalars,
            curves: document.curves,
            contacts: document.contacts,
            trim_views: Vec::new(),
            constraints: document
                .constraints
                .into_iter()
                .map(DocumentConstraint::from)
                .collect(),
            dimensions: document
                .dimensions
                .into_iter()
                .map(DocumentDimension::from)
                .collect(),
            parameters: Vec::new(),
            parameter_bindings: Vec::new(),
            parameter_outputs: Vec::new(),
            external_bindings: Vec::new(),
            source_order: document.source_order,
            geometry_roles: BTreeMap::new(),
            user_inactive_elements: BTreeSet::new(),
            host_activation: None,
            semantic_source_reservations: BTreeMap::new(),
            mutation_validation_deferred: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SketchDocumentV4 {
    version: u32,
    id: DocumentId,
    next_id: PersistentId,
    model_scale: f64,
    points: Vec<DesignPoint>,
    scalars: Vec<DesignScalar>,
    curves: Vec<DesignCurve>,
    contacts: Vec<ContactSlot>,
    trim_views: Vec<DocumentCurveTrimView>,
    constraints: Vec<DocumentConstraintV4>,
    dimensions: Vec<DocumentDimensionV4>,
    source_order: Vec<DocumentSourceId>,
}

impl SketchDocumentV4 {
    fn with_sources(
        document: &SketchDocument,
        constraints: Vec<DocumentConstraintV4>,
        dimensions: Vec<DocumentDimensionV4>,
    ) -> Self {
        Self {
            version: document.version,
            id: document.id,
            next_id: document.next_id,
            model_scale: document.model_scale,
            points: document.points.clone(),
            scalars: document.scalars.clone(),
            curves: document.curves.clone(),
            contacts: document.contacts.clone(),
            trim_views: document.trim_views.clone(),
            constraints,
            dimensions,
            source_order: document.source_order.clone(),
        }
    }
}

impl TryFrom<&SketchDocument> for SketchDocumentV4 {
    type Error = DocumentError;

    fn try_from(document: &SketchDocument) -> Result<Self, Self::Error> {
        let constraints = document
            .constraints
            .iter()
            .map(DocumentConstraintV4::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let dimensions = document
            .dimensions
            .iter()
            .map(DocumentDimensionV4::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self::with_sources(document, constraints, dimensions))
    }
}

impl From<SketchDocumentV4> for SketchDocument {
    fn from(document: SketchDocumentV4) -> Self {
        Self {
            version: document.version,
            id: document.id,
            next_id: document.next_id,
            model_scale: document.model_scale,
            points: document.points,
            scalars: document.scalars,
            curves: document.curves,
            contacts: document.contacts,
            trim_views: document.trim_views,
            constraints: document
                .constraints
                .into_iter()
                .map(DocumentConstraint::from)
                .collect(),
            dimensions: document
                .dimensions
                .into_iter()
                .map(DocumentDimension::from)
                .collect(),
            parameters: Vec::new(),
            parameter_bindings: Vec::new(),
            parameter_outputs: Vec::new(),
            external_bindings: Vec::new(),
            source_order: document.source_order,
            geometry_roles: BTreeMap::new(),
            user_inactive_elements: BTreeSet::new(),
            host_activation: None,
            semantic_source_reservations: BTreeMap::new(),
            mutation_validation_deferred: false,
        }
    }
}

// Explicitly unsupported and intentionally private wire language. Public draft codec
// methods below are doc-hidden so M41 state can round-trip without claiming v5 support.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SketchDocumentDraftV5 {
    version: u32,
    document: SketchDocumentV4,
    geometry_roles: Vec<DraftGeometryRole>,
    user_inactive_elements: Vec<DocumentElementId>,
    host_activation: Option<HostConfigurationActivation>,
    #[serde(default)]
    parameters: Vec<DocumentParameter>,
    #[serde(default)]
    parameter_bindings: Vec<DocumentParameterBinding>,
    #[serde(default)]
    parameter_outputs: Vec<DocumentParameterOutput>,
    #[serde(default)]
    external_bindings: Vec<DocumentExternalBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    retained_planar_constraints: Vec<DraftRetainedPlanarConstraint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    profile_offset_dimensions: Vec<DraftProfileOffsetDimension>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DraftProfileOffsetDimension {
    id: DocumentDimensionId,
    source_id: DocumentSourceId,
    label: String,
    suppressed: bool,
    target: DesignScalarId,
    operand: DocumentProfileOffsetOperand,
}

impl DraftProfileOffsetDimension {
    fn try_from_dimension(value: &DocumentDimension) -> Result<Self, DocumentError> {
        let DocumentDimensionDefinition::ProfileOffset { target, operand } = &value.definition
        else {
            return invalid(
                "draft profile-offset dimension",
                "side-table entries must be profile offsets",
            );
        };
        if value.mode != DocumentDimensionMode::Driving {
            return invalid(
                "draft profile-offset dimension",
                "profile offsets must remain driving",
            );
        }
        Ok(Self {
            id: value.id,
            source_id: value.source_id,
            label: value.label.clone(),
            suppressed: value.suppressed,
            target: *target,
            operand: operand.clone(),
        })
    }
}

impl From<DraftProfileOffsetDimension> for DocumentDimension {
    fn from(value: DraftProfileOffsetDimension) -> Self {
        Self {
            id: value.id,
            source_id: value.source_id,
            label: value.label,
            mode: DocumentDimensionMode::Driving,
            suppressed: value.suppressed,
            definition: DocumentDimensionDefinition::ProfileOffset {
                target: value.target,
                operand: value.operand,
            },
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DraftRetainedPlanarConstraint {
    id: DocumentConstraintId,
    source_id: DocumentSourceId,
    label: String,
    suppressed: bool,
    definition: DraftRetainedPlanarConstraintDefinition,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum DraftRetainedPlanarConstraintDefinition {
    CoincidentWithOrigin {
        point: DesignPointId,
    },
    PointOnDatumAxis {
        point: DesignPointId,
        axis: DocumentCoordinateAxis,
    },
    HorizontalPoints {
        first: DesignPointId,
        second: DesignPointId,
    },
    VerticalPoints {
        first: DesignPointId,
        second: DesignPointId,
    },
    HorizontalPointToMidpoint {
        point: DesignPointId,
        line: CurveSpan,
    },
    VerticalPointToMidpoint {
        point: DesignPointId,
        line: CurveSpan,
    },
    Concentric {
        first: DocumentCenterRef,
        second: DocumentCenterRef,
    },
    Collinear {
        first: DocumentLineSupportRef,
        second: DocumentLineSupportRef,
    },
    CollinearWithDatumAxis {
        line: DocumentLineSupportRef,
        axis: DocumentCoordinateAxis,
    },
    SymmetricAboutDatumAxis {
        first: DesignPointId,
        second: DesignPointId,
        axis: DocumentCoordinateAxis,
    },
}

impl DraftRetainedPlanarConstraint {
    fn from_constraint(value: &DocumentConstraint) -> Option<Self> {
        let definition = match value.definition {
            DocumentConstraintDefinition::CoincidentWithOrigin { point } => {
                DraftRetainedPlanarConstraintDefinition::CoincidentWithOrigin { point }
            }
            DocumentConstraintDefinition::PointOnDatumAxis { point, axis } => {
                DraftRetainedPlanarConstraintDefinition::PointOnDatumAxis { point, axis }
            }
            DocumentConstraintDefinition::HorizontalPoints { first, second } => {
                DraftRetainedPlanarConstraintDefinition::HorizontalPoints { first, second }
            }
            DocumentConstraintDefinition::VerticalPoints { first, second } => {
                DraftRetainedPlanarConstraintDefinition::VerticalPoints { first, second }
            }
            DocumentConstraintDefinition::HorizontalPointToMidpoint { point, line } => {
                DraftRetainedPlanarConstraintDefinition::HorizontalPointToMidpoint { point, line }
            }
            DocumentConstraintDefinition::VerticalPointToMidpoint { point, line } => {
                DraftRetainedPlanarConstraintDefinition::VerticalPointToMidpoint { point, line }
            }
            DocumentConstraintDefinition::Concentric { first, second } => {
                DraftRetainedPlanarConstraintDefinition::Concentric { first, second }
            }
            DocumentConstraintDefinition::Collinear { first, second } => {
                DraftRetainedPlanarConstraintDefinition::Collinear { first, second }
            }
            DocumentConstraintDefinition::CollinearWithDatumAxis { line, axis } => {
                DraftRetainedPlanarConstraintDefinition::CollinearWithDatumAxis { line, axis }
            }
            DocumentConstraintDefinition::SymmetricAboutDatumAxis {
                first,
                second,
                axis,
            } => DraftRetainedPlanarConstraintDefinition::SymmetricAboutDatumAxis {
                first,
                second,
                axis,
            },
            _ => return None,
        };
        Some(Self {
            id: value.id,
            source_id: value.source_id,
            label: value.label.clone(),
            suppressed: value.suppressed,
            definition,
        })
    }
}

impl From<DraftRetainedPlanarConstraint> for DocumentConstraint {
    fn from(value: DraftRetainedPlanarConstraint) -> Self {
        let definition = match value.definition {
            DraftRetainedPlanarConstraintDefinition::CoincidentWithOrigin { point } => {
                DocumentConstraintDefinition::CoincidentWithOrigin { point }
            }
            DraftRetainedPlanarConstraintDefinition::PointOnDatumAxis { point, axis } => {
                DocumentConstraintDefinition::PointOnDatumAxis { point, axis }
            }
            DraftRetainedPlanarConstraintDefinition::HorizontalPoints { first, second } => {
                DocumentConstraintDefinition::HorizontalPoints { first, second }
            }
            DraftRetainedPlanarConstraintDefinition::VerticalPoints { first, second } => {
                DocumentConstraintDefinition::VerticalPoints { first, second }
            }
            DraftRetainedPlanarConstraintDefinition::HorizontalPointToMidpoint { point, line } => {
                DocumentConstraintDefinition::HorizontalPointToMidpoint { point, line }
            }
            DraftRetainedPlanarConstraintDefinition::VerticalPointToMidpoint { point, line } => {
                DocumentConstraintDefinition::VerticalPointToMidpoint { point, line }
            }
            DraftRetainedPlanarConstraintDefinition::Concentric { first, second } => {
                DocumentConstraintDefinition::Concentric { first, second }
            }
            DraftRetainedPlanarConstraintDefinition::Collinear { first, second } => {
                DocumentConstraintDefinition::Collinear { first, second }
            }
            DraftRetainedPlanarConstraintDefinition::CollinearWithDatumAxis { line, axis } => {
                DocumentConstraintDefinition::CollinearWithDatumAxis { line, axis }
            }
            DraftRetainedPlanarConstraintDefinition::SymmetricAboutDatumAxis {
                first,
                second,
                axis,
            } => DocumentConstraintDefinition::SymmetricAboutDatumAxis {
                first,
                second,
                axis,
            },
        };
        Self {
            id: value.id,
            source_id: value.source_id,
            label: value.label,
            suppressed: value.suppressed,
            definition,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DraftGeometryRole {
    curve: CurveId,
    role: GeometryRole,
}

#[derive(Deserialize)]
struct DocumentHeader {
    version: u32,
}

impl SketchDocument {
    pub(crate) fn validate_parameter_scalar_value(
        &self,
        scalar: DesignScalarId,
        value: f64,
    ) -> Result<(), DocumentError> {
        let scalar = self.scalar(scalar).ok_or(DocumentError::UnknownId {
            kind: "scalar",
            id: scalar.0,
        })?;
        validate_scalar_value(value, scalar.domain)
    }

    pub(crate) fn validate_parameter_dimension_value(
        &self,
        dimension: DocumentDimensionId,
        value: f64,
    ) -> Result<(), DocumentError> {
        let dimension = self.dimension(dimension).ok_or(DocumentError::UnknownId {
            kind: "dimension",
            id: dimension.0,
        })?;
        self.validate_parameter_scalar_value(dimension_target(&dimension.definition), value)
    }

    /// Creates an empty current-version document.
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
            trim_views: Vec::new(),
            constraints: Vec::new(),
            dimensions: Vec::new(),
            parameters: Vec::new(),
            parameter_bindings: Vec::new(),
            parameter_outputs: Vec::new(),
            external_bindings: Vec::new(),
            source_order: Vec::new(),
            geometry_roles: BTreeMap::new(),
            user_inactive_elements: BTreeSet::new(),
            host_activation: None,
            semantic_source_reservations: BTreeMap::new(),
            mutation_validation_deferred: false,
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

    /// Returns one curve's persistent profile/construction role.
    #[must_use]
    pub fn geometry_role(&self, curve: CurveId) -> Option<GeometryRole> {
        self.curve(curve)
            .map(|_| self.geometry_roles.get(&curve).copied().unwrap_or_default())
    }

    /// Atomically changes a curve role without changing any geometric or discrete state.
    ///
    /// # Errors
    ///
    /// Returns an error when the curve is unknown or the resulting document is invalid.
    pub fn set_geometry_role(
        &mut self,
        curve: CurveId,
        role: GeometryRole,
    ) -> Result<(), DocumentError> {
        self.set_geometry_roles(&[GeometryRoleEdit { curve, role }])
    }

    /// Atomically changes several curve roles without changing geometry or discrete state.
    ///
    /// Input order is retained by the corresponding command effect. A curve may occur only
    /// once; duplicate entries, including entries that request conflicting roles, are rejected
    /// before any role changes.
    ///
    /// # Errors
    ///
    /// Returns an error when the batch is empty, a curve is unknown, a curve occurs more than
    /// once, or the resulting document is invalid.
    pub fn set_geometry_roles(&mut self, edits: &[GeometryRoleEdit]) -> Result<(), DocumentError> {
        if edits.is_empty() {
            return invalid("geometry role edits", "batch must not be empty");
        }
        let mut curves = BTreeSet::new();
        for edit in edits {
            if self.curve(edit.curve).is_none() {
                return Err(unknown("curve", edit.curve.0));
            }
            if !curves.insert(edit.curve) {
                return invalid(
                    "geometry role edits",
                    format!("curve {} occurs more than once", edit.curve),
                );
            }
        }
        let mut candidate = self.clone();
        for edit in edits {
            if edit.role == GeometryRole::Profile {
                candidate.geometry_roles.remove(&edit.curve);
            } else {
                candidate.geometry_roles.insert(edit.curve, edit.role);
            }
        }
        candidate.validate_after_mutation()?;
        *self = candidate;
        Ok(())
    }

    /// Returns the retained immutable host-configuration activation payload.
    #[must_use]
    pub const fn host_configuration_activation(&self) -> Option<&HostConfigurationActivation> {
        self.host_activation.as_ref()
    }

    /// Atomically installs a newer immutable host-configuration activation payload.
    ///
    /// # Errors
    ///
    /// Returns an error for stale revisions, unknown elements, or invalid resulting state.
    pub fn set_host_configuration_activation(
        &mut self,
        activation: HostConfigurationActivation,
    ) -> Result<(), DocumentError> {
        let retained = self
            .host_activation
            .as_ref()
            .map_or(0, HostConfigurationActivation::revision);
        if activation.revision() <= retained {
            return Err(DocumentError::StaleActivationRevision {
                actual: activation.revision(),
                retained,
            });
        }
        for entry in activation.overrides() {
            if !self.contains_element(entry.element()) {
                return Err(unknown(
                    entry.element().kind(),
                    entry.element().persistent_id(),
                ));
            }
        }
        let mut candidate = self.clone();
        candidate.host_activation = Some(activation);
        candidate.validate_after_mutation()?;
        *self = candidate;
        Ok(())
    }

    /// Atomically changes the requested user suppression of any persistent element.
    ///
    /// Constraint/dimension/source edits update the frozen v1-v4 `suppressed` Boolean
    /// exactly; other element kinds are represented only by draft M41 state.
    ///
    /// # Errors
    ///
    /// Returns an error when the element is unknown or the resulting document is invalid.
    pub fn set_element_user_suppressed(
        &mut self,
        element: DocumentElementId,
        suppressed: bool,
    ) -> Result<(), DocumentError> {
        if !self.contains_element(element) {
            return Err(unknown(element.kind(), element.persistent_id()));
        }
        let mut candidate = self.clone();
        match element {
            DocumentElementId::Constraint(id) => {
                let Some(constraint) = candidate
                    .constraints
                    .iter_mut()
                    .find(|value| value.id == id)
                else {
                    return Err(unknown("constraint", id.0));
                };
                constraint.suppressed = suppressed;
            }
            DocumentElementId::Dimension(id) => {
                let Some(dimension) = candidate.dimensions.iter_mut().find(|value| value.id == id)
                else {
                    return Err(unknown("dimension", id.0));
                };
                dimension.suppressed = suppressed;
            }
            DocumentElementId::Source(id) => {
                if let Some(constraint) = candidate
                    .constraints
                    .iter_mut()
                    .find(|value| value.source_id == id)
                {
                    constraint.suppressed = suppressed;
                } else {
                    let Some(dimension) = candidate
                        .dimensions
                        .iter_mut()
                        .find(|value| value.source_id == id)
                    else {
                        return Err(unknown("source", id.0));
                    };
                    dimension.suppressed = suppressed;
                }
            }
            _ => {
                if suppressed {
                    candidate.user_inactive_elements.insert(element);
                } else {
                    candidate.user_inactive_elements.remove(&element);
                }
            }
        }
        candidate.validate_after_mutation()?;
        *self = candidate;
        Ok(())
    }

    /// Computes the one canonical typed dependency closure used by M41 consumers.
    #[must_use]
    pub fn effective_activity(&self) -> EffectiveActivity {
        self.compute_effective_activity()
    }

    /// Returns the deterministic transitive dependency closure for one persistent element.
    ///
    /// The queried element itself is excluded. The result contains only persistent identities,
    /// is deduplicated, and follows canonical document identity order. Consumers may use this
    /// read-only graph to explain ownership without reproducing constraint definitions.
    #[must_use]
    pub fn dependency_closure(
        &self,
        element: impl Into<DocumentElementId>,
    ) -> Vec<DocumentElementId> {
        let root = element.into();
        let mut discovered = BTreeSet::new();
        let mut pending = self.direct_dependencies(root);
        while let Some(candidate) = pending.pop() {
            if candidate == root || !discovered.insert(candidate) {
                continue;
            }
            pending.extend(self.direct_dependencies(candidate));
        }
        let mut closure = discovered.into_iter().collect::<Vec<_>>();
        closure.sort_by_key(|dependency| canonical_element_key(*dependency));
        closure
    }

    /// Returns every persistent element transitively affected by the queried element.
    ///
    /// This is the reverse of [`Self::dependency_closure`]: the queried element is
    /// excluded, and each returned element directly or transitively depends on it.
    /// Results are deduplicated in canonical persistent-identity order.
    #[must_use]
    pub fn dependent_closure(
        &self,
        element: impl Into<DocumentElementId>,
    ) -> Vec<DocumentElementId> {
        let root = element.into();
        let elements = self.canonical_elements();
        let mut discovered = BTreeSet::new();
        let mut pending = vec![root];
        while let Some(dependency) = pending.pop() {
            for candidate in &elements {
                if *candidate == root || discovered.contains(candidate) {
                    continue;
                }
                if self.direct_dependencies(*candidate).contains(&dependency) {
                    discovered.insert(*candidate);
                    pending.push(*candidate);
                }
            }
        }
        let mut closure = discovered.into_iter().collect::<Vec<_>>();
        closure.sort_by_key(|dependent| canonical_element_key(*dependent));
        closure
    }

    pub(crate) fn effective_activity_with_input_overlays(
        &self,
        parameter_inactive: &BTreeSet<DocumentElementId>,
        unavailable_external: &BTreeSet<DocumentElementId>,
    ) -> EffectiveActivity {
        self.compute_effective_activity_with_input_overlays(
            parameter_inactive,
            unavailable_external,
        )
    }

    #[must_use]
    pub fn contacts(&self) -> &[ContactSlot] {
        &self.contacts
    }

    /// Returns all persistent visible-interval views in canonical support order.
    #[must_use]
    pub fn trim_views(&self) -> &[DocumentCurveTrimView] {
        &self.trim_views
    }

    /// Returns the sole persistent trim view for one immutable support span.
    ///
    /// Multi-interval supports deliberately return `None`; callers that support
    /// general operation topology use [`Self::trim_views_for_span`].
    #[must_use]
    pub fn trim_view(&self, support: CurveSpan) -> Option<&DocumentCurveTrimView> {
        let mut views = self
            .trim_views
            .iter()
            .filter(|view| view.support == support);
        let view = views.next()?;
        views.next().is_none().then_some(view)
    }

    /// Returns every persistent visible interval for one immutable support.
    pub fn trim_views_for_span(
        &self,
        support: CurveSpan,
    ) -> impl Iterator<Item = &DocumentCurveTrimView> {
        self.trim_views
            .iter()
            .filter(move |view| view.support == support)
    }

    /// Resolves one support span's accepted visible interval and boundary provenance.
    ///
    /// Absence of a persistent view resolves to the complete native span.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid support or malformed/dangling boundary.
    pub fn visible_interval(
        &self,
        support: CurveSpan,
    ) -> Result<DocumentVisibleCurveInterval, DocumentError> {
        let intervals = self.visible_intervals(support)?;
        let [interval] = intervals.as_slice() else {
            return invalid(
                "visible curve interval",
                "support has multiple visible intervals; use visible_intervals",
            );
        };
        Ok(*interval)
    }

    /// Resolves every visible interval for one immutable support in traversal order.
    ///
    /// Absence of persistent views resolves to the complete native span.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid, overlapping, or dangling interval state.
    pub fn visible_intervals(
        &self,
        support: CurveSpan,
    ) -> Result<Vec<DocumentVisibleCurveInterval>, DocumentError> {
        self.validate_span(support)?;
        let mut intervals = self
            .trim_views_for_span(support)
            .map(|view| self.resolve_trim_view(view))
            .collect::<Result<Vec<_>, _>>()?;
        if !intervals.is_empty() {
            intervals.sort_by(|first, second| {
                first
                    .start
                    .total_cmp(&second.start)
                    .then(first.end.total_cmp(&second.end))
            });
            return Ok(intervals);
        }
        let period = self.trim_support_period(support)?;
        let periodic = self.trim_support_is_periodic(support)?;
        let start_parameter = DocumentTrimParameter {
            parameter: 0.0,
            winding: 0,
        };
        let end_parameter = DocumentTrimParameter {
            parameter: if periodic { 0.0 } else { period },
            winding: i32::from(periodic),
        };
        Ok(vec![DocumentVisibleCurveInterval {
            support,
            start: 0.0,
            end: period,
            start_boundary: DocumentTrimBoundary::Fixed(start_parameter),
            end_boundary: DocumentTrimBoundary::Fixed(end_parameter),
        }])
    }

    /// Resolves every accepted visible support interval for one curve in semantic order.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing curve or malformed trim view.
    pub fn visible_curve_intervals(
        &self,
        curve: CurveId,
    ) -> Result<Vec<DocumentVisibleCurveInterval>, DocumentError> {
        let mut intervals = Vec::new();
        for span in self.curve_spans(curve)? {
            intervals.extend(self.visible_intervals(span)?);
        }
        Ok(intervals)
    }

    /// Returns whether one unwrapped support parameter is inside the accepted visible interval.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-finite parameter, invalid support, or malformed trim view.
    pub fn is_parameter_visible(
        &self,
        support: CurveSpan,
        total_parameter: f64,
    ) -> Result<bool, DocumentError> {
        finite(total_parameter, "visible curve parameter")?;
        Ok(self
            .visible_intervals(support)?
            .iter()
            .any(|interval| total_parameter >= interval.start && total_parameter <= interval.end))
    }

    /// Atomically replaces every equation-free visible interval for one support.
    ///
    /// Each supplied view must name `support`. Empty input restores the complete
    /// native support. Association-owned boundaries cannot be removed through
    /// this generic operation seam.
    ///
    /// # Errors
    ///
    /// Rejects invalid supports, mismatched/overlapping intervals, or removal of
    /// an association-owned boundary.
    pub fn replace_trim_views(
        &mut self,
        support: CurveSpan,
        views: Vec<DocumentCurveTrimView>,
    ) -> Result<(), DocumentError> {
        self.validate_span(support)?;
        if views.iter().any(|view| view.support != support) {
            return invalid(
                "trim view support",
                "every replacement interval must name the selected support",
            );
        }
        if self.trim_views_for_span(support).any(|view| {
            !matches!(view.start, DocumentTrimBoundary::Fixed(_))
                || !matches!(view.end, DocumentTrimBoundary::Fixed(_))
        }) {
            return invalid(
                "trim view ownership",
                "association-owned trim boundaries cannot be replaced",
            );
        }
        let mut candidate = self.clone();
        candidate.trim_views.retain(|view| view.support != support);
        candidate.trim_views.extend(views);
        candidate.validate_after_mutation()?;
        *self = candidate;
        Ok(())
    }

    /// Removes one fully fixed trim view while preserving immutable support geometry.
    ///
    /// # Errors
    ///
    /// Rejects a missing view or either association-owned boundary.
    pub fn clear_fixed_trim_view(&mut self, support: CurveSpan) -> Result<(), DocumentError> {
        let mut candidate = self.clone();
        let index = candidate
            .trim_views
            .iter()
            .position(|view| view.support == support)
            .ok_or_else(|| DocumentError::InvalidField {
                field: "trim view",
                message: "support has no persistent trim view".into(),
            })?;
        let view = candidate.trim_views[index];
        if !matches!(view.start, DocumentTrimBoundary::Fixed(_))
            || !matches!(view.end, DocumentTrimBoundary::Fixed(_))
        {
            return invalid(
                "trim view",
                "association-owned trim boundaries cannot be cleared",
            );
        }
        candidate.trim_views.remove(index);
        candidate.validate_after_mutation()?;
        *self = candidate;
        Ok(())
    }

    #[must_use]
    pub fn constraints(&self) -> &[DocumentConstraint] {
        &self.constraints
    }

    /// Returns the deterministic representative of every persistent point under active,
    /// explicit [`DocumentConstraintDefinition::Coincident`] topology.
    ///
    /// Coordinate equality, solver tolerance and non-coincidence equations never weld points.
    /// Every point is present in the result and maps to the lowest persistent identity in its
    /// transitive active Coincident component. Consumers can therefore recognize semantic joins
    /// without rebuilding constraint-activation policy or inferring topology from solved geometry.
    #[must_use]
    pub fn point_coincidence_representatives(&self) -> BTreeMap<DesignPointId, DesignPointId> {
        fn root(parents: &mut [usize], value: usize) -> usize {
            let mut representative = value;
            while parents[representative] != representative {
                representative = parents[representative];
            }
            let mut current = value;
            while parents[current] != current {
                let next = parents[current];
                parents[current] = representative;
                current = next;
            }
            representative
        }

        let mut points = self.points.iter().map(|point| point.id).collect::<Vec<_>>();
        points.sort_unstable();
        let indices = points
            .iter()
            .enumerate()
            .map(|(index, point)| (*point, index))
            .collect::<BTreeMap<_, _>>();
        let mut parents = (0..points.len()).collect::<Vec<_>>();
        let activity = self.compute_effective_activity();
        for constraint in self
            .constraints
            .iter()
            .filter(|constraint| activity.is_active(constraint.id))
        {
            let DocumentConstraintDefinition::Coincident { first, second } = constraint.definition
            else {
                continue;
            };
            let (Some(first), Some(second)) = (indices.get(&first), indices.get(&second)) else {
                continue;
            };
            let first = root(&mut parents, *first);
            let second = root(&mut parents, *second);
            if first != second {
                let (representative, child) = if first < second {
                    (first, second)
                } else {
                    (second, first)
                };
                parents[child] = representative;
            }
        }
        points
            .iter()
            .enumerate()
            .map(|(index, point)| (*point, points[root(&mut parents, index)]))
            .collect()
    }

    /// Returns the active association that derives one circular arc's fillet endpoints.
    #[must_use]
    pub fn line_line_fillet_for_arc(&self, arc: CurveId) -> Option<&DocumentConstraint> {
        let activity = self.compute_effective_activity();
        self.line_line_fillet_owner_for_arc(arc)
            .filter(|constraint| activity.is_active(constraint.id))
    }

    fn line_line_fillet_owner_for_arc(&self, arc: CurveId) -> Option<&DocumentConstraint> {
        self.constraints.iter().find(|constraint| {
            matches!(
                constraint.definition,
                DocumentConstraintDefinition::LineLineFillet { arc: output, .. }
                    if output == arc
            )
        })
    }

    /// Returns the active generic association that derives one circular arc's fillet endpoints.
    #[must_use]
    pub fn curve_curve_fillet_for_arc(&self, arc: CurveId) -> Option<&DocumentConstraint> {
        let activity = self.compute_effective_activity();
        self.constraints.iter().find(|constraint| {
            activity.is_active(constraint.id)
                && matches!(
                    constraint.definition,
                    DocumentConstraintDefinition::CurveCurveFillet { arc: output, .. }
                        if output == arc
                )
        })
    }

    #[must_use]
    pub fn dimensions(&self) -> &[DocumentDimension] {
        &self.dimensions
    }

    /// Returns persistent host-parameter declarations in canonical identity order.
    #[must_use]
    pub fn parameters(&self) -> &[DocumentParameter] {
        &self.parameters
    }

    /// Resolves one persistent host-parameter declaration.
    #[must_use]
    pub fn parameter(&self, id: DocumentParameterId) -> Option<&DocumentParameter> {
        self.parameters.iter().find(|parameter| parameter.id == id)
    }

    /// Returns persistent host-input associations in canonical pair order.
    #[must_use]
    pub fn parameter_bindings(&self) -> &[DocumentParameterBinding] {
        &self.parameter_bindings
    }

    /// Returns declared reference-output proposals in canonical pair order.
    #[must_use]
    pub fn parameter_outputs(&self) -> &[DocumentParameterOutput] {
        &self.parameter_outputs
    }

    /// Returns external-reference declarations in canonical identity order.
    #[must_use]
    pub fn external_bindings(&self) -> &[DocumentExternalBinding] {
        &self.external_bindings
    }

    /// Resolves one document-local external binding.
    #[must_use]
    pub fn external_binding(
        &self,
        id: DocumentExternalBindingId,
    ) -> Option<&DocumentExternalBinding> {
        self.external_bindings
            .iter()
            .find(|binding| binding.id == id)
    }

    /// Allocates one persistent external-reference declaration.
    ///
    /// # Errors
    ///
    /// Rejects invalid labels, kind/topology combinations, exhausted identities, and
    /// external-binding resource-limit violations.
    pub fn add_external_binding(
        &mut self,
        label: impl Into<String>,
        expected_kind: ExternalFeatureKindV1,
        expected_topology: Option<ExternalTopologyDigest>,
    ) -> Result<DocumentExternalBindingId, DocumentError> {
        let label = label.into();
        validate_label(&label, "external binding label")?;
        validate_external_binding_shape(expected_kind, expected_topology)?;
        if self.external_bindings.len() >= MAX_EXTERNAL_BINDINGS {
            return Err(DocumentError::ResourceLimit {
                resource: "external bindings",
                actual: self.external_bindings.len() + 1,
                limit: MAX_EXTERNAL_BINDINGS,
            });
        }
        let mut candidate = self.clone();
        let id = DocumentExternalBindingId(candidate.allocate_id()?);
        candidate.external_bindings.push(DocumentExternalBinding {
            id,
            label,
            expected_kind,
            expected_topology,
        });
        candidate.validate_after_mutation()?;
        candidate.canonicalize();
        *self = candidate;
        Ok(id)
    }

    /// Explicitly changes the family/topology contract of one retained binding.
    ///
    /// # Errors
    ///
    /// Rejects an unknown binding or an invalid kind/topology combination.
    pub fn rebind_external_binding(
        &mut self,
        id: DocumentExternalBindingId,
        expected_kind: ExternalFeatureKindV1,
        expected_topology: Option<ExternalTopologyDigest>,
    ) -> Result<(), DocumentError> {
        validate_external_binding_shape(expected_kind, expected_topology)?;
        let mut candidate = self.clone();
        let binding = candidate
            .external_bindings
            .iter_mut()
            .find(|binding| binding.id == id)
            .ok_or_else(|| unknown("external binding", id.0))?;
        binding.expected_kind = expected_kind;
        binding.expected_topology = expected_topology;
        candidate.validate_after_mutation()?;
        *self = candidate;
        Ok(())
    }

    /// Allocates one persistent host-parameter declaration.
    ///
    /// # Errors
    ///
    /// Rejects an invalid label, exhausted identity space, or resource limit.
    pub fn add_parameter(
        &mut self,
        label: impl Into<String>,
        kind: DocumentParameterKind,
    ) -> Result<DocumentParameterId, DocumentError> {
        let label = label.into();
        validate_label(&label, "parameter label")?;
        if self.parameters.len() >= MAX_DOCUMENT_PARAMETERS {
            return Err(DocumentError::ResourceLimit {
                resource: "parameters",
                actual: self.parameters.len() + 1,
                limit: MAX_DOCUMENT_PARAMETERS,
            });
        }
        let mut candidate = self.clone();
        let id = DocumentParameterId(candidate.allocate_id()?);
        candidate
            .parameters
            .push(DocumentParameter { id, label, kind });
        candidate.validate_after_mutation()?;
        *self = candidate;
        Ok(id)
    }

    /// Adds one typed host-input association atomically.
    ///
    /// # Errors
    ///
    /// Rejects missing identities, duplicate targets, incompatible kinds, or local
    /// input/output ownership overlap.
    pub fn add_parameter_binding(
        &mut self,
        parameter: DocumentParameterId,
        target: DocumentParameterTarget,
    ) -> Result<(), DocumentError> {
        let mut candidate = self.clone();
        candidate
            .parameter_bindings
            .push(DocumentParameterBinding { parameter, target });
        candidate.validate_after_mutation()?;
        candidate.canonicalize();
        *self = candidate;
        Ok(())
    }

    /// Removes one exact host-input association.
    ///
    /// # Errors
    ///
    /// Rejects a missing association or an invalid resulting document.
    pub fn remove_parameter_binding(
        &mut self,
        parameter: DocumentParameterId,
        target: DocumentParameterTarget,
    ) -> Result<(), DocumentError> {
        let mut candidate = self.clone();
        let index = candidate
            .parameter_bindings
            .iter()
            .position(|binding| binding.parameter == parameter && binding.target == target)
            .ok_or_else(|| invalid_error("parameter binding", "association does not exist"))?;
        candidate.parameter_bindings.remove(index);
        candidate.validate_after_mutation()?;
        *self = candidate;
        Ok(())
    }

    /// Declares one reference-dimension output proposal atomically.
    ///
    /// # Errors
    ///
    /// Rejects missing identities, incompatible output declarations, or local
    /// input/output ownership overlap.
    pub fn add_parameter_output(
        &mut self,
        parameter: DocumentParameterId,
        dimension: DocumentDimensionId,
    ) -> Result<(), DocumentError> {
        let mut candidate = self.clone();
        candidate.parameter_outputs.push(DocumentParameterOutput {
            parameter,
            dimension,
        });
        candidate.validate_after_mutation()?;
        candidate.canonicalize();
        *self = candidate;
        Ok(())
    }

    /// Removes one exact reference-output declaration.
    ///
    /// # Errors
    ///
    /// Rejects a missing declaration or an invalid resulting document.
    pub fn remove_parameter_output(
        &mut self,
        parameter: DocumentParameterId,
        dimension: DocumentDimensionId,
    ) -> Result<(), DocumentError> {
        let mut candidate = self.clone();
        let index = candidate
            .parameter_outputs
            .iter()
            .position(|output| output.parameter == parameter && output.dimension == dimension)
            .ok_or_else(|| invalid_error("parameter output", "declaration does not exist"))?;
        candidate.parameter_outputs.remove(index);
        candidate.validate_after_mutation()?;
        *self = candidate;
        Ok(())
    }

    #[must_use]
    pub fn source_order(&self) -> &[DocumentSourceId] {
        &self.source_order
    }

    /// Resolves one persistent audit source to its domain owner.
    #[must_use]
    pub fn source(&self, id: DocumentSourceId) -> Option<DocumentSourceRef<'_>> {
        if let Some(constraint) = self
            .constraints
            .iter()
            .find(|constraint| constraint.source_id == id)
        {
            return Some(DocumentSourceRef {
                id,
                owner: DocumentSourceOwner::Constraint(constraint.id),
                label: &constraint.label,
                suppressed: constraint.suppressed,
            });
        }
        self.dimensions
            .iter()
            .find(|dimension| dimension.source_id == id)
            .map(|dimension| DocumentSourceRef {
                id,
                owner: DocumentSourceOwner::Dimension(dimension.id),
                label: &dimension.label,
                suppressed: dimension.suppressed,
            })
    }

    /// Iterates persistent sources in semantic equation/audit order.
    pub fn sources(&self) -> impl Iterator<Item = DocumentSourceRef<'_>> + '_ {
        self.source_order
            .iter()
            .filter_map(|source| self.source(*source))
    }

    /// Returns whether one typed persistent element currently belongs to the document.
    #[must_use]
    pub fn contains_element(&self, element: DocumentElementId) -> bool {
        match element {
            DocumentElementId::Document(id) => self.id == id,
            DocumentElementId::Point(id) => self.point(id).is_some(),
            DocumentElementId::Scalar(id) => self.scalar(id).is_some(),
            DocumentElementId::Curve(id) => self.curve(id).is_some(),
            DocumentElementId::Contact(id) => self.contact(id).is_some(),
            DocumentElementId::Constraint(id) => self.constraint(id).is_some(),
            DocumentElementId::Dimension(id) => self.dimension(id).is_some(),
            DocumentElementId::Parameter(id) => self.parameter(id).is_some(),
            DocumentElementId::ExternalBinding(id) => self.external_binding(id).is_some(),
            DocumentElementId::Source(id) => self.source(id).is_some(),
        }
    }

    /// Resolves a raw persistent identity to its unique typed document element.
    #[must_use]
    pub fn element(&self, id: PersistentId) -> Option<DocumentElementId> {
        if self.id.0 == id {
            return Some(DocumentElementId::Document(self.id));
        }
        self.points
            .iter()
            .find_map(|value| (value.id.0 == id).then_some(DocumentElementId::Point(value.id)))
            .or_else(|| {
                self.scalars.iter().find_map(|value| {
                    (value.id.0 == id).then_some(DocumentElementId::Scalar(value.id))
                })
            })
            .or_else(|| {
                self.curves.iter().find_map(|value| {
                    (value.id.0 == id).then_some(DocumentElementId::Curve(value.id))
                })
            })
            .or_else(|| {
                self.contacts.iter().find_map(|value| {
                    (value.id.0 == id).then_some(DocumentElementId::Contact(value.id))
                })
            })
            .or_else(|| {
                self.constraints.iter().find_map(|value| {
                    (value.id.0 == id).then_some(DocumentElementId::Constraint(value.id))
                })
            })
            .or_else(|| {
                self.dimensions.iter().find_map(|value| {
                    (value.id.0 == id).then_some(DocumentElementId::Dimension(value.id))
                })
            })
            .or_else(|| {
                self.parameters.iter().find_map(|value| {
                    (value.id.0 == id).then_some(DocumentElementId::Parameter(value.id))
                })
            })
            .or_else(|| {
                self.external_bindings.iter().find_map(|value| {
                    (value.id.0 == id).then_some(DocumentElementId::ExternalBinding(value.id))
                })
            })
            .or_else(|| {
                self.source_order.iter().find_map(|source| {
                    (source.0 == id).then_some(DocumentElementId::Source(*source))
                })
            })
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

    pub(crate) fn spline_basis(
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

    pub(crate) fn bspline_geometry(
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

    pub(crate) fn nurbs_geometry(
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

    pub(crate) fn spline_span_index(
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

    /// Returns the conventional or projective middle-control view of one rational conic.
    ///
    /// Nonzero weights expose the Euclidean control `P1 = Qh / w`. A zero weight has no finite
    /// Euclidean control and therefore exposes the raw homogeneous vector `Qh` explicitly.
    ///
    /// # Errors
    ///
    /// Returns a typed failure for a missing/wrong-family curve or an unrepresentable quotient.
    pub fn rational_conic_control(
        &self,
        curve: CurveId,
    ) -> Result<DocumentRationalConicControl, DocumentCurveControlError> {
        let id = DocumentCurveControlId {
            curve,
            kind: DocumentCurveControlKind::RationalMiddle,
        };
        let CurveDefinition::RationalQuadraticConic {
            weighted_middle,
            middle_weight,
            ..
        } = &self
            .curve(curve)
            .ok_or_else(|| unknown("curve", curve.0))?
            .definition
        else {
            return Err(DocumentCurveControlError::UnknownControl {
                curve,
                kind: id.kind,
            });
        };
        let weight = self.require_scalar(*middle_weight)?.value;
        if weight == 0.0 {
            return Ok(DocumentRationalConicControl::Projective {
                weighted_middle: *weighted_middle,
                weight: 0.0,
            });
        }
        let middle = [weighted_middle[0] / weight, weighted_middle[1] / weight];
        if !middle.iter().all(|value| value.is_finite()) {
            return Err(DocumentCurveControlError::NonFiniteResult { control: id });
        }
        Ok(DocumentRationalConicControl::Euclidean { middle, weight })
    }

    /// Enumerates the finite transient controls owned by one native curve.
    ///
    /// Point-backed controls retain their persistent point aliases. Derived endpoints and size
    /// grips retain only stable curve-local identities and scalar targets; they never become
    /// document points or snapping/constraint operands.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a missing curve or invalid/unrepresentable accepted geometry.
    #[allow(clippy::too_many_lines)]
    pub fn curve_controls(
        &self,
        curve: CurveId,
    ) -> Result<Vec<DocumentCurveControl>, DocumentCurveControlError> {
        let value = self.curve(curve).ok_or_else(|| unknown("curve", curve.0))?;
        let activity = self.compute_effective_activity();
        let base_availability = self.curve_control_availability(curve, &activity);
        let point_position = |point: DesignPointId| -> Result<[f64; 2], DocumentError> {
            Ok(self.require_point(point)?.position)
        };
        let endpoint_position = |endpoint: FeatureEndpoint| {
            let parameter = match endpoint {
                FeatureEndpoint::Start => 0.0,
                FeatureEndpoint::End => 1.0,
            };
            let point = self
                .evaluate_curve_jet(CurveSpan::line(curve), parameter)?
                .position;
            Ok::<_, DocumentCurveControlError>([point.x, point.y])
        };
        let mut controls = Vec::new();
        match &value.definition {
            CurveDefinition::Line { start, end, .. } => {
                push_curve_control(
                    &mut controls,
                    curve,
                    DocumentCurveControlKind::StartPoint,
                    point_position(*start)?,
                    DocumentCurveControlTarget::Point(*start),
                    base_availability,
                )?;
                push_curve_control(
                    &mut controls,
                    curve,
                    DocumentCurveControlKind::EndPoint,
                    point_position(*end)?,
                    DocumentCurveControlTarget::Point(*end),
                    base_availability,
                )?;
            }
            CurveDefinition::Polyline { points, closed, .. } => {
                for (index, point) in points.iter().copied().enumerate() {
                    let ordinal =
                        u32::try_from(index).map_err(|_| DocumentError::ResourceLimit {
                            resource: "curve control ordinal",
                            actual: index,
                            limit: u32::MAX as usize,
                        })?;
                    let kind = if !closed && index == 0 {
                        DocumentCurveControlKind::StartPoint
                    } else if !closed && index + 1 == points.len() {
                        DocumentCurveControlKind::EndPoint
                    } else {
                        DocumentCurveControlKind::ControlPoint { ordinal }
                    };
                    push_curve_control(
                        &mut controls,
                        curve,
                        kind,
                        point_position(point)?,
                        DocumentCurveControlTarget::Point(point),
                        base_availability,
                    )?;
                }
            }
            CurveDefinition::Circle { center, radius } => {
                let center_position = point_position(*center)?;
                let radius_value = self.require_scalar(*radius)?.value;
                push_curve_control(
                    &mut controls,
                    curve,
                    DocumentCurveControlKind::Center,
                    center_position,
                    DocumentCurveControlTarget::Point(*center),
                    base_availability,
                )?;
                push_curve_control(
                    &mut controls,
                    curve,
                    DocumentCurveControlKind::Radius,
                    [center_position[0] + radius_value, center_position[1]],
                    DocumentCurveControlTarget::Scalar(*radius),
                    self.radial_control_availability(curve, *radius, base_availability, &activity),
                )?;
            }
            CurveDefinition::CircularArc { center, radius, .. } => {
                push_curve_control(
                    &mut controls,
                    curve,
                    DocumentCurveControlKind::Center,
                    point_position(*center)?,
                    DocumentCurveControlTarget::Point(*center),
                    base_availability,
                )?;
                let midpoint = self
                    .evaluate_curve_jet(CurveSpan::line(curve), 0.5)?
                    .position;
                push_curve_control(
                    &mut controls,
                    curve,
                    DocumentCurveControlKind::Radius,
                    [midpoint.x, midpoint.y],
                    DocumentCurveControlTarget::Scalar(*radius),
                    self.radial_control_availability(curve, *radius, base_availability, &activity),
                )?;
                push_trim_controls(
                    self,
                    &mut controls,
                    curve,
                    endpoint_position(FeatureEndpoint::Start)?,
                    endpoint_position(FeatureEndpoint::End)?,
                    &value.definition,
                    base_availability,
                )?;
            }
            CurveDefinition::QuadraticBezier { controls: points } => {
                for (index, point) in points.iter().copied().enumerate() {
                    let ordinal =
                        u32::try_from(index).map_err(|_| DocumentError::ResourceLimit {
                            resource: "curve control ordinal",
                            actual: index,
                            limit: u32::MAX as usize,
                        })?;
                    push_curve_control(
                        &mut controls,
                        curve,
                        DocumentCurveControlKind::ControlPoint { ordinal },
                        point_position(point)?,
                        DocumentCurveControlTarget::Point(point),
                        base_availability,
                    )?;
                }
            }
            CurveDefinition::CubicBezier { controls: points } => {
                for (index, point) in points.iter().copied().enumerate() {
                    let ordinal =
                        u32::try_from(index).map_err(|_| DocumentError::ResourceLimit {
                            resource: "curve control ordinal",
                            actual: index,
                            limit: u32::MAX as usize,
                        })?;
                    push_curve_control(
                        &mut controls,
                        curve,
                        DocumentCurveControlKind::ControlPoint { ordinal },
                        point_position(point)?,
                        DocumentCurveControlTarget::Point(point),
                        base_availability,
                    )?;
                }
            }
            CurveDefinition::Ellipse {
                center,
                major_axis_point,
                minor_axis_ratio,
            } => {
                push_axis_controls(
                    self,
                    &mut controls,
                    curve,
                    *center,
                    *major_axis_point,
                    *minor_axis_ratio,
                    base_availability,
                )?;
            }
            CurveDefinition::EllipticalArc {
                center,
                major_axis_point,
                minor_axis_ratio,
                ..
            } => {
                push_axis_controls(
                    self,
                    &mut controls,
                    curve,
                    *center,
                    *major_axis_point,
                    *minor_axis_ratio,
                    base_availability,
                )?;
                push_trim_controls(
                    self,
                    &mut controls,
                    curve,
                    endpoint_position(FeatureEndpoint::Start)?,
                    endpoint_position(FeatureEndpoint::End)?,
                    &value.definition,
                    base_availability,
                )?;
            }
            CurveDefinition::RationalQuadraticConic {
                start,
                middle_weight,
                end,
                ..
            } => {
                push_curve_control(
                    &mut controls,
                    curve,
                    DocumentCurveControlKind::StartPoint,
                    point_position(*start)?,
                    DocumentCurveControlTarget::Point(*start),
                    base_availability,
                )?;
                let rational = self.rational_conic_control(curve)?;
                let middle_position = match rational {
                    DocumentRationalConicControl::Euclidean { middle, .. } => middle,
                    DocumentRationalConicControl::Projective {
                        weighted_middle, ..
                    } => {
                        let anchor = point_position(*start)?;
                        [
                            anchor[0] + weighted_middle[0],
                            anchor[1] + weighted_middle[1],
                        ]
                    }
                };
                push_curve_control(
                    &mut controls,
                    curve,
                    DocumentCurveControlKind::RationalMiddle,
                    middle_position,
                    DocumentCurveControlTarget::RationalMiddle {
                        weight: *middle_weight,
                        mode: rational.mode(),
                    },
                    base_availability,
                )?;
                push_curve_control(
                    &mut controls,
                    curve,
                    DocumentCurveControlKind::EndPoint,
                    point_position(*end)?,
                    DocumentCurveControlTarget::Point(*end),
                    base_availability,
                )?;
            }
            CurveDefinition::ParabolaSegment { vertex, focus, .. } => {
                push_curve_control(
                    &mut controls,
                    curve,
                    DocumentCurveControlKind::Vertex,
                    point_position(*vertex)?,
                    DocumentCurveControlTarget::Point(*vertex),
                    base_availability,
                )?;
                push_curve_control(
                    &mut controls,
                    curve,
                    DocumentCurveControlKind::Focus,
                    point_position(*focus)?,
                    DocumentCurveControlTarget::Point(*focus),
                    base_availability,
                )?;
                push_trim_controls(
                    self,
                    &mut controls,
                    curve,
                    endpoint_position(FeatureEndpoint::Start)?,
                    endpoint_position(FeatureEndpoint::End)?,
                    &value.definition,
                    base_availability,
                )?;
            }
            CurveDefinition::HyperbolaSegment {
                center,
                transverse_axis_point,
                semi_conjugate,
                ..
            } => {
                let center_position = point_position(*center)?;
                let transverse_position = point_position(*transverse_axis_point)?;
                push_curve_control(
                    &mut controls,
                    curve,
                    DocumentCurveControlKind::Center,
                    center_position,
                    DocumentCurveControlTarget::Point(*center),
                    base_availability,
                )?;
                push_curve_control(
                    &mut controls,
                    curve,
                    DocumentCurveControlKind::TransverseAxisPoint,
                    transverse_position,
                    DocumentCurveControlTarget::Point(*transverse_axis_point),
                    base_availability,
                )?;
                let transverse = point_difference(center_position, transverse_position);
                let length = transverse[0].hypot(transverse[1]);
                let conjugate = self.require_scalar(*semi_conjugate)?.value;
                let conjugate_position = [
                    center_position[0] - transverse[1] * conjugate / length,
                    center_position[1] + transverse[0] * conjugate / length,
                ];
                push_curve_control(
                    &mut controls,
                    curve,
                    DocumentCurveControlKind::ConjugateAxis,
                    conjugate_position,
                    DocumentCurveControlTarget::Scalar(*semi_conjugate),
                    self.scalar_control_availability(*semi_conjugate, base_availability),
                )?;
                push_trim_controls(
                    self,
                    &mut controls,
                    curve,
                    endpoint_position(FeatureEndpoint::Start)?,
                    endpoint_position(FeatureEndpoint::End)?,
                    &value.definition,
                    base_availability,
                )?;
            }
            CurveDefinition::BSpline {
                controls: points, ..
            }
            | CurveDefinition::Nurbs {
                controls: points, ..
            } => {
                for (index, point) in points.iter().copied().enumerate() {
                    let ordinal =
                        u32::try_from(index).map_err(|_| DocumentError::ResourceLimit {
                            resource: "curve control ordinal",
                            actual: index,
                            limit: u32::MAX as usize,
                        })?;
                    push_curve_control(
                        &mut controls,
                        curve,
                        DocumentCurveControlKind::ControlPoint { ordinal },
                        point_position(point)?,
                        DocumentCurveControlTarget::Point(point),
                        base_availability,
                    )?;
                }
            }
        }
        Ok(controls)
    }

    /// Inverse-projects one transient curve control onto its persistent document target.
    ///
    /// This performs no mutation. Applying the returned point/scalar/rational edit through an
    /// ordinary session still performs complete solve and independent residual validation.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a stale/foreign/read-only control, non-finite target, or an
    /// invalid/unrepresentable inverse mapping.
    pub fn project_curve_control(
        &self,
        control: DocumentCurveControlId,
        target: [f64; 2],
    ) -> Result<DocumentCurveControlProjection, DocumentCurveControlError> {
        if !target.iter().all(|value| value.is_finite()) {
            return Err(DocumentCurveControlError::NonFiniteTarget { control });
        }
        let view = self
            .curve_controls(control.curve)?
            .into_iter()
            .find(|candidate| candidate.id == control)
            .ok_or(DocumentCurveControlError::UnknownControl {
                curve: control.curve,
                kind: control.kind,
            })?;
        if let DocumentCurveControlAvailability::ReadOnly(reason) = view.availability {
            return Err(DocumentCurveControlError::ReadOnly { control, reason });
        }
        match view.target {
            DocumentCurveControlTarget::Point(point) => Ok(DocumentCurveControlProjection::Point {
                point,
                position: target,
            }),
            DocumentCurveControlTarget::Scalar(scalar) => {
                let value = match control.kind {
                    DocumentCurveControlKind::TrimStart | DocumentCurveControlKind::TrimEnd => {
                        let endpoint = if control.kind == DocumentCurveControlKind::TrimStart {
                            FeatureEndpoint::Start
                        } else {
                            FeatureEndpoint::End
                        };
                        let projection =
                            self.project_curve_trim_endpoint(control.curve, endpoint, target)?;
                        debug_assert_eq!(projection.scalar, scalar);
                        projection.value
                    }
                    DocumentCurveControlKind::Radius => {
                        self.project_radius_control(control, target)?
                    }
                    DocumentCurveControlKind::MinorAxis => {
                        self.project_minor_axis_control(control, target)?
                    }
                    DocumentCurveControlKind::ConjugateAxis => {
                        self.project_conjugate_axis_control(control, target)?
                    }
                    _ => {
                        return Err(DocumentCurveControlError::UnknownControl {
                            curve: control.curve,
                            kind: control.kind,
                        });
                    }
                };
                if !value.is_finite() {
                    return Err(DocumentCurveControlError::NonFiniteResult { control });
                }
                Ok(DocumentCurveControlProjection::Scalar { scalar, value })
            }
            DocumentCurveControlTarget::RationalMiddle { mode, .. } => {
                let current = self.rational_conic_control(control.curve)?;
                let rational = match (mode, current) {
                    (
                        DocumentRationalConicControlMode::Euclidean,
                        DocumentRationalConicControl::Euclidean { weight, .. },
                    ) => {
                        rational_weighted_middle_preserving_control(target, weight)?;
                        DocumentRationalConicControl::Euclidean {
                            middle: target,
                            weight,
                        }
                    }
                    (
                        DocumentRationalConicControlMode::Projective,
                        DocumentRationalConicControl::Projective { weight, .. },
                    ) => {
                        let CurveDefinition::RationalQuadraticConic { start, .. } = &self
                            .curve(control.curve)
                            .ok_or_else(|| unknown("curve", control.curve.0))?
                            .definition
                        else {
                            unreachable!("rational control target came from another family")
                        };
                        let anchor = self.require_point(*start)?.position;
                        let weighted_middle = [target[0] - anchor[0], target[1] - anchor[1]];
                        if !weighted_middle.iter().all(|value| value.is_finite()) {
                            return Err(DocumentCurveControlError::NonFiniteResult { control });
                        }
                        DocumentRationalConicControl::Projective {
                            weighted_middle,
                            weight,
                        }
                    }
                    _ => {
                        return Err(DocumentCurveControlError::UnknownControl {
                            curve: control.curve,
                            kind: control.kind,
                        });
                    }
                };
                Ok(DocumentCurveControlProjection::RationalMiddle {
                    curve: control.curve,
                    control: rational,
                })
            }
        }
    }

    fn curve_control_availability(
        &self,
        curve: CurveId,
        activity: &EffectiveActivity,
    ) -> DocumentCurveControlAvailability {
        if self.curve_is_active_fillet_output(curve, activity) {
            DocumentCurveControlAvailability::ReadOnly(
                DocumentCurveControlWithholdingReason::AssociativeFilletOutput,
            )
        } else if !activity.is_active(curve) {
            DocumentCurveControlAvailability::ReadOnly(
                DocumentCurveControlWithholdingReason::InactiveCurve,
            )
        } else {
            DocumentCurveControlAvailability::Editable
        }
    }

    fn scalar_control_availability(
        &self,
        scalar: DesignScalarId,
        fallback: DocumentCurveControlAvailability,
    ) -> DocumentCurveControlAvailability {
        if fallback != DocumentCurveControlAvailability::Editable {
            return fallback;
        }
        if self.parameter_bindings.iter().any(|binding| {
            matches!(
                binding.target,
                DocumentParameterTarget::DimensionlessFixedScalar(property)
                    if property.scalar == scalar
            )
        }) {
            DocumentCurveControlAvailability::ReadOnly(
                DocumentCurveControlWithholdingReason::HostParameterOwned,
            )
        } else {
            fallback
        }
    }

    fn radial_control_availability(
        &self,
        curve: CurveId,
        scalar: DesignScalarId,
        fallback: DocumentCurveControlAvailability,
        activity: &EffectiveActivity,
    ) -> DocumentCurveControlAvailability {
        let scalar_availability = self.scalar_control_availability(scalar, fallback);
        if scalar_availability != DocumentCurveControlAvailability::Editable {
            return scalar_availability;
        }
        if self.dimensions.iter().any(|dimension| {
            activity.is_active(dimension.id)
                && dimension.mode == DocumentDimensionMode::Driving
                && matches!(
                    dimension.definition,
                    DocumentDimensionDefinition::Radius {
                        curve: dimension_curve,
                        ..
                    } | DocumentDimensionDefinition::Diameter {
                        curve: dimension_curve,
                        ..
                    } if dimension_curve == curve
                )
        }) {
            return DocumentCurveControlAvailability::ReadOnly(
                DocumentCurveControlWithholdingReason::DrivingDimensionOwned,
            );
        }
        if self.constraints.iter().any(|constraint| {
            activity.is_active(constraint.id)
                && matches!(
                    constraint.definition,
                    DocumentConstraintDefinition::EqualRadius { first, second }
                        if first == curve || second == curve
                )
        }) {
            DocumentCurveControlAvailability::ReadOnly(
                DocumentCurveControlWithholdingReason::EqualRadiusOwned,
            )
        } else {
            DocumentCurveControlAvailability::Editable
        }
    }

    fn curve_is_active_fillet_output(&self, curve: CurveId, activity: &EffectiveActivity) -> bool {
        self.constraints.iter().any(|constraint| {
            activity.is_active(constraint.id)
                && matches!(
                    constraint.definition,
                    DocumentConstraintDefinition::LineLineFillet { arc, .. }
                        | DocumentConstraintDefinition::CurveCurveFillet { arc, .. }
                        if arc == curve
                )
        })
    }

    fn project_radius_control(
        &self,
        control: DocumentCurveControlId,
        target: [f64; 2],
    ) -> Result<f64, DocumentCurveControlError> {
        let definition = &self
            .curve(control.curve)
            .ok_or_else(|| unknown("curve", control.curve.0))?
            .definition;
        let center = match definition {
            CurveDefinition::Circle { center, .. }
            | CurveDefinition::CircularArc { center, .. } => self.require_point(*center)?.position,
            _ => {
                return Err(DocumentCurveControlError::UnknownControl {
                    curve: control.curve,
                    kind: control.kind,
                });
            }
        };
        Ok((target[0] - center[0]).hypot(target[1] - center[1]))
    }

    fn project_minor_axis_control(
        &self,
        control: DocumentCurveControlId,
        target: [f64; 2],
    ) -> Result<f64, DocumentCurveControlError> {
        let definition = &self
            .curve(control.curve)
            .ok_or_else(|| unknown("curve", control.curve.0))?
            .definition;
        let (center, major_axis_point) = match definition {
            CurveDefinition::Ellipse {
                center,
                major_axis_point,
                ..
            }
            | CurveDefinition::EllipticalArc {
                center,
                major_axis_point,
                ..
            } => (
                self.require_point(*center)?.position,
                self.require_point(*major_axis_point)?.position,
            ),
            _ => {
                return Err(DocumentCurveControlError::UnknownControl {
                    curve: control.curve,
                    kind: control.kind,
                });
            }
        };
        let major = point_difference(center, major_axis_point);
        let semi_major = major[0].hypot(major[1]);
        let target_vector = point_difference(center, target);
        let semi_minor =
            (-target_vector[0] * major[1] + target_vector[1] * major[0]).abs() / semi_major;
        Ok(semi_minor / semi_major)
    }

    fn project_conjugate_axis_control(
        &self,
        control: DocumentCurveControlId,
        target: [f64; 2],
    ) -> Result<f64, DocumentCurveControlError> {
        let definition = &self
            .curve(control.curve)
            .ok_or_else(|| unknown("curve", control.curve.0))?
            .definition;
        let (center, transverse_axis_point) = match definition {
            CurveDefinition::HyperbolaSegment {
                center,
                transverse_axis_point,
                ..
            } => (
                self.require_point(*center)?.position,
                self.require_point(*transverse_axis_point)?.position,
            ),
            _ => {
                return Err(DocumentCurveControlError::UnknownControl {
                    curve: control.curve,
                    kind: control.kind,
                });
            }
        };
        let transverse = point_difference(center, transverse_axis_point);
        let semi_transverse = transverse[0].hypot(transverse[1]);
        let target_vector = point_difference(center, target);
        Ok(
            (-target_vector[0] * transverse[1] + target_vector[1] * transverse[0]).abs()
                / semi_transverse,
        )
    }

    /// Projects a world target onto one curve's existing start/end trim scalar.
    ///
    /// Angular results are unwrapped near the selected endpoint's current scalar. Non-periodic
    /// trims reject an endpoint crossing that would reverse their existing direction. The method
    /// does not clamp, reorder, swap, allocate, or change explicit sweep/branch state.
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

        let activity = self.compute_effective_activity();
        let definition = &self
            .curve(curve)
            .ok_or_else(|| unknown("curve", curve.0))?
            .definition;
        if self.constraints.iter().any(|constraint| {
            activity.is_active(constraint.id)
                && matches!(
                    constraint.definition,
                    DocumentConstraintDefinition::LineLineFillet { arc, .. }
                        | DocumentConstraintDefinition::CurveCurveFillet { arc, .. }
                        if arc == curve
                )
        }) {
            return Err(DocumentTrimProjectionError::UnsupportedCurve { curve });
        }
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
        if let CurveDefinition::ParabolaSegment {
            trim_start,
            trim_end,
            ..
        }
        | CurveDefinition::HyperbolaSegment {
            trim_start,
            trim_end,
            ..
        } = definition
        {
            let start = self.require_scalar(*trim_start)?.value;
            let end = self.require_scalar(*trim_end)?.value;
            let current_rate = end - start;
            let candidate_rate = match endpoint {
                FeatureEndpoint::Start => end - value,
                FeatureEndpoint::End => value - start,
            };
            if !candidate_rate.is_finite() {
                return Err(DocumentTrimProjectionError::NonFiniteResult { curve });
            }
            if candidate_rate == 0.0
                || candidate_rate.is_sign_negative() != current_rate.is_sign_negative()
            {
                return Err(DocumentTrimProjectionError::CrossesOppositeEndpoint {
                    curve,
                    endpoint,
                });
            }
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
    pub(crate) fn conic_geometry(
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
                match &value.definition {
                    CurveDefinition::Polyline { closed: true, .. } => {
                        return invalid("feature endpoint", "a closed curve has no endpoint");
                    }
                    CurveDefinition::Circle { .. }
                    | CurveDefinition::Ellipse { .. }
                    | CurveDefinition::BSpline {
                        form: DocumentBSplineForm::Periodic,
                        ..
                    }
                    | CurveDefinition::Nurbs {
                        form: DocumentBSplineForm::Periodic,
                        ..
                    } => {
                        return invalid("feature endpoint", "a periodic curve has no endpoint");
                    }
                    _ => {}
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
                    | CurveDefinition::HyperbolaSegment { .. } => 0,
                    CurveDefinition::QuadraticBezier { .. } => 3,
                    CurveDefinition::CubicBezier { .. } => 4,
                    CurveDefinition::BSpline { controls, .. }
                    | CurveDefinition::Nurbs { controls, .. } => controls.len(),
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

    /// Validates one closed point-valued operand through persistent identity.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing identity or unsupported capability.
    pub fn validate_point_ref(&self, point: DocumentPointRef) -> Result<(), DocumentError> {
        let feature = match point {
            DocumentPointRef::Point { point } => FeatureRef::Point { point },
            DocumentPointRef::Center(center) => FeatureRef::CurveCenter {
                curve: center.curve,
            },
            DocumentPointRef::Endpoint(endpoint) => FeatureRef::CurveEndpoint {
                curve: endpoint.curve,
                endpoint: endpoint.endpoint,
            },
            DocumentPointRef::Control(control) => return self.validate_control_ref(control),
            DocumentPointRef::Focus { curve, index } => FeatureRef::CurveFocus { curve, index },
            DocumentPointRef::FixedCurveLocation { contact } => {
                FeatureRef::FixedCurveLocation { contact }
            }
        };
        self.validate_feature(feature)
    }

    /// Validates a center operand without accepting an incidental coordinate match.
    ///
    /// # Errors
    ///
    /// Returns an error when the curve is missing or has no semantic center.
    pub fn validate_center_ref(&self, center: DocumentCenterRef) -> Result<(), DocumentError> {
        self.validate_feature(FeatureRef::CurveCenter {
            curve: center.curve,
        })
    }

    /// Resolves a validated semantic-center operand to its stored design point.
    ///
    /// This exposes semantic identity rather than an evaluated coordinate, so
    /// interaction policy can reject tautological center relations without
    /// reproducing curve-family rules.
    ///
    /// # Errors
    ///
    /// Returns an error when the curve is missing or has no stored semantic center.
    pub fn resolve_center_ref(
        &self,
        center: DocumentCenterRef,
    ) -> Result<DesignPointId, DocumentError> {
        self.validate_center_ref(center)?;
        match &self
            .curve(center.curve)
            .ok_or_else(|| unknown("curve", center.curve.0))?
            .definition
        {
            CurveDefinition::Circle { center, .. }
            | CurveDefinition::CircularArc { center, .. }
            | CurveDefinition::Ellipse { center, .. }
            | CurveDefinition::EllipticalArc { center, .. }
            | CurveDefinition::HyperbolaSegment { center, .. } => Ok(*center),
            _ => invalid("center feature", "curve has no stored semantic center"),
        }
    }

    /// Validates an endpoint operand without manufacturing an endpoint for periodic topology.
    ///
    /// # Errors
    ///
    /// Returns an error when the curve is missing or has no semantic endpoint.
    pub fn validate_endpoint_ref(
        &self,
        endpoint: DocumentEndpointRef,
    ) -> Result<(), DocumentError> {
        self.validate_feature(FeatureRef::CurveEndpoint {
            curve: endpoint.curve,
            endpoint: endpoint.endpoint,
        })
    }

    /// Validates a stored control operand, including B-spline and NURBS controls.
    ///
    /// # Errors
    ///
    /// Returns an error when the curve is missing or the persistent point is not one
    /// of that curve's stored controls.
    pub fn validate_control_ref(&self, control: DocumentControlRef) -> Result<(), DocumentError> {
        self.resolve_control_ref(control).map(|_| ())
    }

    /// Resolves a stored control through its persistent point identity.
    ///
    /// # Errors
    ///
    /// Returns an error when the owning curve is missing, the point is missing, or
    /// the point is not a stored control of that curve.
    pub fn resolve_control_ref(
        &self,
        control: DocumentControlRef,
    ) -> Result<DesignPointId, DocumentError> {
        self.require_point(control.control)?;
        let curve = self
            .curve(control.curve)
            .ok_or_else(|| unknown("curve", control.curve.0))?;
        let controls: &[DesignPointId] = match &curve.definition {
            CurveDefinition::Line { start, end, .. } => {
                if control.control == *start || control.control == *end {
                    return Ok(control.control);
                }
                &[]
            }
            CurveDefinition::Polyline { points, .. }
            | CurveDefinition::BSpline {
                controls: points, ..
            }
            | CurveDefinition::Nurbs {
                controls: points, ..
            } => points,
            CurveDefinition::QuadraticBezier { controls } => controls,
            CurveDefinition::CubicBezier { controls } => controls,
            _ => &[],
        };
        if controls.contains(&control.control) {
            Ok(control.control)
        } else {
            invalid(
                "feature control",
                "persistent point is not a stored control of the owning curve",
            )
        }
    }

    /// Validates one directed line-support operand.
    ///
    /// # Errors
    ///
    /// Returns an error unless the persistent span is a line or polyline segment.
    pub fn validate_line_support_ref(
        &self,
        support: DocumentLineSupportRef,
    ) -> Result<(), DocumentError> {
        self.validate_line_span(support.span)?;
        Ok(())
    }

    /// Validates one branch-explicit direction operand.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing or unsupported semantic direction.
    pub fn validate_direction_ref(
        &self,
        direction: DocumentDirectionRef,
    ) -> Result<(), DocumentError> {
        match direction {
            DocumentDirectionRef::CurveAxis { curve, .. } => {
                self.validate_feature(FeatureRef::CurveAxis { curve })
            }
            DocumentDirectionRef::LineSupport(support) => self.validate_line_support_ref(support),
            DocumentDirectionRef::CurveTangent { contact, .. }
            | DocumentDirectionRef::CurveNormal { contact, .. } => {
                let contact = self.require_contact(contact)?;
                self.contact_differential(contact)?;
                Ok(())
            }
        }
    }

    /// Validates one stable semantic span and its explicit traversal winding.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing span or nonzero winding on non-periodic topology.
    pub fn validate_curve_span_ref(&self, span: DocumentCurveSpanRef) -> Result<(), DocumentError> {
        self.validate_span(span.span)?;
        if span.winding != 0 && !self.trim_support_allows_winding(span.span)? {
            return invalid(
                "curve span winding",
                "non-periodic topology requires zero winding",
            );
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
        self.add_curve_with_role(label, definition, GeometryRole::Profile)
    }

    /// Adds one curve with an explicit persistent profile/construction role.
    ///
    /// Curve creation and role assignment are one atomic document mutation. The legacy
    /// [`Self::add_curve`] API remains equivalent to requesting [`GeometryRole::Profile`].
    ///
    /// # Errors
    ///
    /// Returns an error for invalid data or a missing semantic reference.
    pub fn add_curve_with_role(
        &mut self,
        label: impl Into<String>,
        definition: CurveDefinition,
        role: GeometryRole,
    ) -> Result<CurveId, DocumentError> {
        let label = label.into();
        validate_label(&label, "curve label")?;
        let id = CurveId(self.allocate_id()?);
        self.curves.push(DesignCurve {
            id,
            label,
            definition,
        });
        if role == GeometryRole::Construction {
            self.geometry_roles.insert(id, role);
        }
        if let Err(error) = self.validate() {
            self.curves.pop();
            self.geometry_roles.remove(&id);
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
        if self
            .trim_views
            .iter()
            .any(|view| view.support.curve == curve)
        {
            return invalid(
                "B-spline knot insertion",
                "curve has a persistent trim view",
            );
        }
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
        candidate.validate_after_mutation()?;
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
        if self
            .trim_views
            .iter()
            .any(|view| view.support.curve == curve)
        {
            return invalid("NURBS knot insertion", "curve has a persistent trim view");
        }
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
        candidate.validate_after_mutation()?;
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
        candidate.validate_after_mutation()?;
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
        candidate.validate_after_mutation()?;
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
        let contact_domain = if matches!(
            definition,
            CurveDefinition::Circle { .. } | CurveDefinition::Ellipse { .. }
        ) {
            ContactDomain::Periodic {
                period: std::f64::consts::TAU,
            }
        } else {
            ContactDomain::Bounded {
                lower: 0.0,
                upper: 1.0,
            }
        };
        self.add_curve_contact_with_domain(
            label,
            curve,
            contact_domain,
            parameter,
            winding,
            neighborhood,
            tangent_orientation,
        )
    }

    /// Atomically creates a parameter scalar and contact slot in one explicitly
    /// selected domain supported by the curve span.
    ///
    /// # Errors
    ///
    /// Returns an error for an unsupported domain, invalid span, parameter,
    /// neighborhood, winding, or tangent orientation.
    #[allow(clippy::too_many_arguments)]
    pub fn add_curve_contact_with_domain(
        &mut self,
        label: impl Into<String>,
        curve: CurveSpan,
        domain: ContactDomain,
        parameter: f64,
        winding: i32,
        neighborhood: ContactNeighborhood,
        tangent_orientation: Option<TangentOrientation>,
    ) -> Result<ContactId, DocumentError> {
        let label = label.into();
        if !self.curve_contact_domains(curve)?.contains(&domain) {
            return invalid(
                "contact domain",
                "selected parameter domain is unsupported by the curve span",
            );
        }
        let (unit, scalar_domain) = match domain {
            ContactDomain::SupportingLine => (ScalarUnit::Parameter, ScalarDomain::Finite),
            ContactDomain::Bounded { lower, upper } => (
                ScalarUnit::Parameter,
                ScalarDomain::Bounded { lower, upper },
            ),
            ContactDomain::Periodic { period } => {
                (ScalarUnit::Angle, ScalarDomain::Periodic { period })
            }
        };
        let mut candidate = self.clone();
        let scalar =
            candidate.add_scalar(format!("{label} parameter"), parameter, unit, scalar_domain)?;
        let contact = candidate.add_contact(
            label,
            ContactDefinition {
                curve,
                parameter: scalar,
                domain,
                winding,
                neighborhood,
                tangent_orientation,
            },
        )?;
        *self = candidate;
        Ok(contact)
    }

    /// Returns every parameter-domain topology supported by one semantic curve span.
    ///
    /// Linear spans deliberately expose both their finite segment and unbounded
    /// supporting line. Periodic curves expose only their periodic domain; every
    /// other alpha curve span exposes its finite unit interval.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing curve or invalid span.
    pub fn curve_contact_domains(
        &self,
        curve: CurveSpan,
    ) -> Result<Vec<ContactDomain>, DocumentError> {
        let definition = &self.validate_span(curve)?.definition;
        Ok(match definition {
            CurveDefinition::Line { .. } | CurveDefinition::Polyline { .. } => vec![
                ContactDomain::Bounded {
                    lower: 0.0,
                    upper: 1.0,
                },
                ContactDomain::SupportingLine,
            ],
            CurveDefinition::Circle { .. } | CurveDefinition::Ellipse { .. } => {
                vec![ContactDomain::Periodic {
                    period: std::f64::consts::TAU,
                }]
            }
            _ => vec![ContactDomain::Bounded {
                lower: 0.0,
                upper: 1.0,
            }],
        })
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

    /// Atomically declares one grouped driving profile-offset dimension and its
    /// private positive length scalar. All source and target geometry already exists.
    ///
    /// # Errors
    /// Returns an error without mutation when the distance, topology, exact
    /// junction provenance, curve families, or branch state is invalid.
    pub fn add_profile_offset(
        &mut self,
        label: impl Into<String>,
        distance: f64,
        operand: DocumentProfileOffsetOperand,
    ) -> Result<DocumentProfileOffsetIds, DocumentError> {
        let label = label.into();
        validate_label(&label, "profile offset label")?;
        finite_positive(distance, "profile offset distance")?;
        let mut candidate = self.clone();
        let target = candidate.add_scalar(
            format!("{label} distance"),
            distance,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )?;
        let dimension = candidate.add_dimension(
            label,
            DocumentDimensionDefinition::ProfileOffset { target, operand },
            DocumentDimensionMode::Driving,
        )?;
        candidate.validate_after_mutation()?;
        *self = candidate;
        Ok(DocumentProfileOffsetIds { target, dimension })
    }

    /// Atomically replaces only the retained topology and discrete branch state
    /// of an existing profile offset while preserving its dimension/source IDs.
    ///
    /// # Errors
    /// Returns an error without mutation for a stale/non-profile dimension or an
    /// invalid replacement association.
    pub fn set_profile_offset_operand(
        &mut self,
        dimension: DocumentDimensionId,
        operand: DocumentProfileOffsetOperand,
    ) -> Result<(), DocumentError> {
        let mut candidate = self.clone();
        let value = candidate
            .dimensions
            .iter_mut()
            .find(|value| value.id == dimension)
            .ok_or_else(|| unknown("profile offset dimension", dimension.0))?;
        let DocumentDimensionDefinition::ProfileOffset {
            operand: current, ..
        } = &mut value.definition
        else {
            return invalid(
                "profile offset dimension",
                "the selected dimension is not a profile offset",
            );
        };
        *current = operand;
        candidate.validate_after_mutation()?;
        *self = candidate;
        Ok(())
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
        self.add_rectangle_with_role(label, origin, width, height, GeometryRole::Profile)
    }

    /// Expands a rectangle whose four ordinary curves receive one explicit geometry role.
    ///
    /// Geometry, sources, and roles are created atomically. [`Self::add_rectangle`] remains the
    /// compatibility spelling for a Profile rectangle.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid dimensions or any failed expanded edit.
    pub fn add_rectangle_with_role(
        &mut self,
        label: &str,
        origin: [f64; 2],
        width: f64,
        height: f64,
        role: GeometryRole,
    ) -> Result<RectangleIds, DocumentError> {
        validate_label(label, "rectangle label")?;
        finite_pair(origin, "rectangle origin")?;
        finite_positive(width, "rectangle width")?;
        finite_positive(height, "rectangle height")?;
        let before = self.clone();
        let result = self.add_rectangle_inner(label, origin, width, height, role);
        if result.is_err() {
            let next_id = self.next_id;
            *self = before;
            self.next_id = next_id;
        }
        result
    }

    #[allow(clippy::too_many_lines)]
    fn add_rectangle_inner(
        &mut self,
        label: &str,
        origin: [f64; 2],
        width: f64,
        height: f64,
        role: GeometryRole,
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
                self.add_curve_with_role(
                    format!("{label}.edge_{}", index + 1),
                    CurveDefinition::Line {
                        start: points[start],
                        end: points[end],
                        branch_direction: direction,
                    },
                    role,
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

    /// Creates one associative circular fillet between two bounded directed line spans.
    ///
    /// Parent curves remain untrimmed. Deleting the returned association constraint removes its
    /// contacts and leaves the last accepted arc as ordinary geometry.
    ///
    /// # Errors
    ///
    /// Rejects parallel/unresolved parents, escaped contacts, invalid radius/labels, or any
    /// expanded document that fails complete validation.
    pub fn add_line_line_fillet(
        &mut self,
        label: &str,
        request: LineLineFilletRequest,
    ) -> Result<LineLineFilletIds, DocumentError> {
        validate_label(label, "line fillet label")?;
        finite_positive(request.radius, "line fillet radius")?;
        self.validate_line_span(request.first)?;
        self.validate_line_span(request.second)?;
        if request.first == request.second {
            return invalid("line fillet parent", "line spans must be distinct");
        }
        let before = self.clone();
        let result = self.add_line_line_fillet_inner(label, request);
        if result.is_err() {
            let next_id = self.next_id;
            *self = before;
            self.next_id = next_id;
        }
        result
    }

    #[allow(clippy::too_many_lines)]
    fn add_line_line_fillet_inner(
        &mut self,
        label: &str,
        request: LineLineFilletRequest,
    ) -> Result<LineLineFilletIds, DocumentError> {
        let output_role = if [request.first.curve, request.second.curve]
            .into_iter()
            .any(|curve| self.geometry_role(curve) == Some(GeometryRole::Construction))
        {
            GeometryRole::Construction
        } else {
            GeometryRole::Profile
        };
        let (first_start_id, first_end_id) = self.line_span_endpoint_ids(request.first)?;
        let (second_start_id, second_end_id) = self.line_span_endpoint_ids(request.second)?;
        let first_start = self.require_point(first_start_id)?.position;
        let first_end = self.require_point(first_end_id)?.position;
        let second_start = self.require_point(second_start_id)?.position;
        let second_end = self.require_point(second_end_id)?.position;
        let first_delta = [first_end[0] - first_start[0], first_end[1] - first_start[1]];
        let second_delta = [
            second_end[0] - second_start[0],
            second_end[1] - second_start[1],
        ];
        let first_length = first_delta[0].hypot(first_delta[1]);
        let second_length = second_delta[0].hypot(second_delta[1]);
        if !first_length.is_finite()
            || !second_length.is_finite()
            || first_length == 0.0
            || second_length == 0.0
        {
            return invalid("line fillet parent", "line span is degenerate");
        }
        let first_direction = [first_delta[0] / first_length, first_delta[1] / first_length];
        let second_direction = [
            second_delta[0] / second_length,
            second_delta[1] / second_length,
        ];
        let determinant = cross(first_direction, second_direction);
        if !determinant.is_finite() || determinant.abs() <= 1.0e-8 {
            return invalid(
                "line fillet parent",
                "line directions are parallel or numerically unresolved",
            );
        }
        let first_side = match request.first_side {
            DocumentCurveNormalSide::Left => 1.0,
            DocumentCurveNormalSide::Right => -1.0,
        };
        let second_side = match request.second_side {
            DocumentCurveNormalSide::Left => 1.0,
            DocumentCurveNormalSide::Right => -1.0,
        };
        let first_normal = [-first_direction[1], first_direction[0]];
        let second_normal = [-second_direction[1], second_direction[0]];
        let first_offset_origin = [
            first_start[0] + first_side * request.radius * first_normal[0],
            first_start[1] + first_side * request.radius * first_normal[1],
        ];
        let second_offset_origin = [
            second_start[0] + second_side * request.radius * second_normal[0],
            second_start[1] + second_side * request.radius * second_normal[1],
        ];
        let offset_difference = [
            second_offset_origin[0] - first_offset_origin[0],
            second_offset_origin[1] - first_offset_origin[1],
        ];
        let first_distance = cross(offset_difference, second_direction) / determinant;
        let center_position = [
            first_offset_origin[0] + first_distance * first_direction[0],
            first_offset_origin[1] + first_distance * first_direction[1],
        ];
        finite_pair(center_position, "line fillet center")?;
        let first_contact_position = [
            center_position[0] - first_side * request.radius * first_normal[0],
            center_position[1] - first_side * request.radius * first_normal[1],
        ];
        let second_contact_position = [
            center_position[0] - second_side * request.radius * second_normal[0],
            center_position[1] - second_side * request.radius * second_normal[1],
        ];
        let first_parameter = dot(
            [
                first_contact_position[0] - first_start[0],
                first_contact_position[1] - first_start[1],
            ],
            first_direction,
        ) / first_length;
        let second_parameter = dot(
            [
                second_contact_position[0] - second_start[0],
                second_contact_position[1] - second_start[1],
            ],
            second_direction,
        ) / second_length;
        if !first_parameter.is_finite()
            || !second_parameter.is_finite()
            || first_parameter <= 0.0
            || first_parameter >= 1.0
            || second_parameter <= 0.0
            || second_parameter >= 1.0
        {
            return invalid(
                "line fillet contact",
                "selected radius/side root escapes a strict parent interior",
            );
        }
        let first_angle = (first_contact_position[1] - center_position[1])
            .atan2(first_contact_position[0] - center_position[0]);
        let second_angle = (second_contact_position[1] - center_position[1])
            .atan2(second_contact_position[0] - center_position[0]);
        let (start_value, end_value) = match request.endpoint_order {
            DocumentFilletEndpointOrder::FirstThenSecond => (first_angle, second_angle),
            DocumentFilletEndpointOrder::SecondThenFirst => (second_angle, first_angle),
        };
        document_arc_signed_sweep(start_value, end_value, request.sweep)?;

        let center = self.add_named_point(format!("{label}.center"), center_position)?;
        let radius = self.add_scalar(
            format!("{label}.radius"),
            request.radius,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )?;
        let start_angle = self.add_scalar(
            format!("{label}.start_angle"),
            start_value,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )?;
        let end_angle = self.add_scalar(
            format!("{label}.end_angle"),
            end_value,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )?;
        let arc = self.add_curve_with_role(
            format!("{label}.arc"),
            CurveDefinition::CircularArc {
                center,
                radius,
                start_angle,
                end_angle,
                sweep: request.sweep,
            },
            output_role,
        )?;
        let contact_parameters = [
            self.add_scalar(
                format!("{label}.first_parameter"),
                first_parameter,
                ScalarUnit::Parameter,
                ScalarDomain::Bounded {
                    lower: 0.0,
                    upper: 1.0,
                },
            )?,
            self.add_scalar(
                format!("{label}.second_parameter"),
                second_parameter,
                ScalarUnit::Parameter,
                ScalarDomain::Bounded {
                    lower: 0.0,
                    upper: 1.0,
                },
            )?,
        ];
        let contacts = [
            self.add_contact(
                format!("{label}.first_contact"),
                ContactDefinition {
                    curve: request.first,
                    parameter: contact_parameters[0],
                    domain: ContactDomain::Bounded {
                        lower: 0.0,
                        upper: 1.0,
                    },
                    winding: 0,
                    neighborhood: ContactNeighborhood::Interior,
                    tangent_orientation: None,
                },
            )?,
            self.add_contact(
                format!("{label}.second_contact"),
                ContactDefinition {
                    curve: request.second,
                    parameter: contact_parameters[1],
                    domain: ContactDomain::Bounded {
                        lower: 0.0,
                        upper: 1.0,
                    },
                    winding: 0,
                    neighborhood: ContactNeighborhood::Interior,
                    tangent_orientation: None,
                },
            )?,
        ];
        let constraint = self.add_constraint(
            format!("{label}.association"),
            DocumentConstraintDefinition::LineLineFillet {
                arc,
                first_contact: contacts[0],
                first_side: request.first_side,
                second_contact: contacts[1],
                second_side: request.second_side,
                endpoint_order: request.endpoint_order,
            },
        )?;
        let radius_target = self.add_scalar(
            format!("{label}.radius_target"),
            request.radius,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )?;
        let radius_dimension = self.add_dimension(
            format!("{label}.radius_dimension"),
            DocumentDimensionDefinition::Radius {
                curve: arc,
                target: radius_target,
            },
            request.radius_mode,
        )?;
        Ok(LineLineFilletIds {
            constraint,
            arc,
            center,
            radius,
            start_angle,
            end_angle,
            contacts,
            contact_parameters,
            radius_dimension,
            radius_target,
        })
    }

    /// Creates one associative circular fillet between two regular immutable curve supports.
    ///
    /// The construction creates one contact-owned trim view per parent without changing either
    /// support definition. Full circles and ellipses require explicit periodic anchors.
    ///
    /// # Errors
    ///
    /// Rejects duplicate or already-trimmed supports, endpoint/ambiguous roots, irregular or
    /// tangent parent jets, unresolved offset factors, invalid periodic anchors, or any expanded
    /// document that fails complete validation.
    pub fn add_curve_curve_fillet(
        &mut self,
        label: &str,
        request: CurveCurveFilletRequest,
    ) -> Result<CurveCurveFilletIds, DocumentError> {
        validate_label(label, "curve fillet label")?;
        finite_positive(request.radius, "curve fillet radius")?;
        if request.first.curve == request.second.curve {
            return invalid("curve fillet parent", "support spans must be distinct");
        }
        for parent in [request.first, request.second] {
            self.validate_fillet_parent_request(parent)?;
            if self.trim_views_for_span(parent.curve).next().is_some() {
                return invalid(
                    "curve fillet parent",
                    "support already has a persistent trim view",
                );
            }
        }
        let before = self.clone();
        let result = self.add_curve_curve_fillet_inner(label, request);
        if result.is_err() {
            let next_id = self.next_id;
            *self = before;
            self.next_id = next_id;
        }
        result
    }

    #[allow(clippy::too_many_lines)]
    fn add_curve_curve_fillet_inner(
        &mut self,
        label: &str,
        request: CurveCurveFilletRequest,
    ) -> Result<CurveCurveFilletIds, DocumentError> {
        let output_role = if [request.first.curve.curve, request.second.curve.curve]
            .into_iter()
            .any(|curve| self.geometry_role(curve) == Some(GeometryRole::Construction))
        {
            GeometryRole::Construction
        } else {
            GeometryRole::Profile
        };
        let first_jet = self.validate_fillet_parent_request(request.first)?;
        let second_jet = self.validate_fillet_parent_request(request.second)?;
        let first_differential =
            first_jet
                .differential()
                .map_err(|source| DocumentError::InvalidField {
                    field: "curve fillet parent",
                    message: source.to_string(),
                })?;
        let second_differential =
            second_jet
                .differential()
                .map_err(|source| DocumentError::InvalidField {
                    field: "curve fillet parent",
                    message: source.to_string(),
                })?;
        let tangent_cross = cross(
            [
                first_differential.unit_tangent.x,
                first_differential.unit_tangent.y,
            ],
            [
                second_differential.unit_tangent.x,
                second_differential.unit_tangent.y,
            ],
        );
        if !tangent_cross.is_finite() || tangent_cross.abs() <= 1.0e-8 {
            return invalid(
                "curve fillet parent",
                "parent tangents are parallel or numerically unresolved",
            );
        }
        let first_sign = fillet_side_sign(request.first.side);
        let second_sign = fillet_side_sign(request.second.side);
        for (sign, curvature) in [
            (first_sign, first_differential.signed_curvature),
            (second_sign, second_differential.signed_curvature),
        ] {
            let factor = 1.0 - sign * request.radius * curvature;
            if !factor.is_finite() || factor.abs() <= 1.0e-8 {
                return invalid(
                    "curve fillet parent",
                    "parent offset factor is numerically unresolved",
                );
            }
        }
        let offset_center = |jet: geosolve_geometry::CurveJet2,
                             differential: geosolve_geometry::CurveDifferential2,
                             sign: f64|
         -> [f64; 2] {
            [
                jet.position.x + sign * request.radius * differential.left_normal.x,
                jet.position.y + sign * request.radius * differential.left_normal.y,
            ]
        };
        let first_center = offset_center(first_jet, first_differential, first_sign);
        let second_center = offset_center(second_jet, second_differential, second_sign);
        let center_position = [
            0.5 * (first_center[0] + second_center[0]),
            0.5 * (first_center[1] + second_center[1]),
        ];
        finite_pair(center_position, "curve fillet center")?;
        let contact_angle =
            |position: geosolve_geometry::Point2<f64>| -> Result<f64, DocumentError> {
                let offset = [
                    position.x - center_position[0],
                    position.y - center_position[1],
                ];
                validate_direction(offset, "curve fillet radial seed")?;
                Ok(offset[1].atan2(offset[0]))
            };
        let first_angle = contact_angle(first_jet.position)?;
        let second_angle = contact_angle(second_jet.position)?;
        let (start_value, end_value) = match request.endpoint_order {
            DocumentFilletEndpointOrder::FirstThenSecond => (first_angle, second_angle),
            DocumentFilletEndpointOrder::SecondThenFirst => (second_angle, first_angle),
        };
        document_arc_signed_sweep(start_value, end_value, request.sweep)?;

        let center = self.add_named_point(format!("{label}.center"), center_position)?;
        let radius = self.add_scalar(
            format!("{label}.radius"),
            request.radius,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )?;
        let start_angle = self.add_scalar(
            format!("{label}.start_angle"),
            start_value,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )?;
        let end_angle = self.add_scalar(
            format!("{label}.end_angle"),
            end_value,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )?;
        let arc = self.add_curve_with_role(
            format!("{label}.arc"),
            CurveDefinition::CircularArc {
                center,
                radius,
                start_angle,
                end_angle,
                sweep: request.sweep,
            },
            output_role,
        )?;
        let contacts = [
            self.add_curve_contact(
                format!("{label}.first_contact"),
                request.first.curve,
                request.first.parameter,
                request.first.winding,
                request.first.neighborhood,
                None,
            )?,
            self.add_curve_contact(
                format!("{label}.second_contact"),
                request.second.curve,
                request.second.parameter,
                request.second.winding,
                request.second.neighborhood,
                None,
            )?,
        ];
        let contact_parameters = [
            self.require_contact(contacts[0])?.parameter,
            self.require_contact(contacts[1])?.parameter,
        ];
        let constraint = DocumentConstraintId(self.allocate_id()?);
        let source_id = DocumentSourceId(self.allocate_id()?);
        self.constraints.push(DocumentConstraint {
            id: constraint,
            source_id,
            label: format!("{label}.association"),
            suppressed: false,
            definition: DocumentConstraintDefinition::CurveCurveFillet {
                arc,
                first_contact: contacts[0],
                first_side: request.first.side,
                first_trim_endpoint: request.first.trim_endpoint,
                second_contact: contacts[1],
                second_side: request.second.side,
                second_trim_endpoint: request.second.trim_endpoint,
                endpoint_order: request.endpoint_order,
            },
        });
        self.source_order.push(source_id);
        self.trim_views.extend([
            self.fillet_trim_view(request.first, constraint, contacts[0])?,
            self.fillet_trim_view(request.second, constraint, contacts[1])?,
        ]);
        self.validate_after_mutation()?;

        let radius_target = self.add_scalar(
            format!("{label}.radius_target"),
            request.radius,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )?;
        let radius_dimension = self.add_dimension(
            format!("{label}.radius_dimension"),
            DocumentDimensionDefinition::Radius {
                curve: arc,
                target: radius_target,
            },
            request.radius_mode,
        )?;
        Ok(CurveCurveFilletIds {
            constraint,
            arc,
            center,
            radius,
            start_angle,
            end_angle,
            contacts,
            contact_parameters,
            radius_dimension,
            radius_target,
        })
    }

    /// Mirrors one point-defined curve about a directed line span.
    ///
    /// The result is ordinary geometry associated by one ordinary point-symmetry constraint per
    /// control point. Supported sources are lines, polylines, quadratic/cubic Beziers, and
    /// non-rational B-splines.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid axis, unsupported source, or failed atomic expanded edit.
    pub fn add_mirrored_curve(
        &mut self,
        label: &str,
        source_curve: CurveId,
        axis: CurveSpan,
    ) -> Result<MirroredCurveIds, DocumentError> {
        validate_label(label, "mirrored curve label")?;
        self.validate_line_span(axis)?;
        let before = self.clone();
        let result = self.add_mirrored_curve_inner(label, source_curve, axis);
        if result.is_err() {
            let next_id = self.next_id;
            *self = before;
            self.next_id = next_id;
        }
        result
    }

    fn add_mirrored_curve_inner(
        &mut self,
        label: &str,
        source_curve: CurveId,
        axis: CurveSpan,
    ) -> Result<MirroredCurveIds, DocumentError> {
        let source = self
            .curve(source_curve)
            .ok_or_else(|| unknown("curve", source_curve.0))?
            .clone();
        let source_role = self
            .geometry_role(source_curve)
            .expect("validated source curve has a geometry role");
        let controls = point_defined_curve_controls(&source.definition).ok_or_else(|| {
            DocumentError::InvalidField {
                field: "mirror source",
                message: "expected a line, polyline, Bezier, or non-rational B-spline".into(),
            }
        })?;
        let (axis_start, _) = self.line_span_endpoint_ids(axis)?;
        let axis_origin = self.require_point(axis_start)?.position;
        let axis_direction = self.current_curve_span_direction(axis)?;
        let mut point_pairs = Vec::with_capacity(controls.len());
        for (index, source_point) in controls.iter().copied().enumerate() {
            let position = self.require_point(source_point)?.position;
            let mirrored = reflect_point_about_line(position, axis_origin, axis_direction)?;
            let mirrored_point =
                self.add_named_point(format!("{label}.point_{}", index + 1), mirrored)?;
            point_pairs.push((source_point, mirrored_point));
        }
        let mirrored_controls = point_pairs
            .iter()
            .map(|(_, mirrored)| *mirrored)
            .collect::<Vec<_>>();
        let mirrored_definition =
            mirror_curve_definition(source.definition, &mirrored_controls, axis_direction)?;
        let mirrored_curve =
            self.add_curve_with_role(format!("{label}.curve"), mirrored_definition, source_role)?;
        let symmetry_constraints = point_pairs
            .iter()
            .enumerate()
            .map(|(index, (source_point, mirrored_point))| {
                self.add_constraint(
                    format!("{label}.symmetry_{}", index + 1),
                    DocumentConstraintDefinition::SymmetricAboutLine {
                        first: *source_point,
                        second: *mirrored_point,
                        line: axis,
                    },
                )
            })
            .collect::<Result<Vec<_>, DocumentError>>()?;
        Ok(MirroredCurveIds {
            source_curve,
            mirrored_curve,
            point_pairs,
            symmetry_constraints,
        })
    }

    /// Inserts the same knot into an associated mirrored B-spline pair.
    ///
    /// Both topology edits and the new control-point symmetry constraint are accepted atomically.
    /// The pair must have identical basis topology and an active symmetry constraint for every
    /// corresponding pre-refinement control pair.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid association, insertion, or refined document.
    pub fn insert_mirrored_bspline_knot(
        &mut self,
        label: &str,
        source_curve: CurveId,
        mirrored_curve: CurveId,
        axis: CurveSpan,
        parameter: f64,
    ) -> Result<DocumentMirroredBSplineInsertion, DocumentError> {
        validate_label(label, "mirrored B-spline insertion label")?;
        let activity = self.compute_effective_activity();
        self.validate_mirrored_bspline_pair(source_curve, mirrored_curve, axis, &activity)?;
        let before = self.clone();
        let result = (|| {
            let source = self.insert_bspline_knot(source_curve, parameter)?;
            let mirrored = self.insert_bspline_knot(mirrored_curve, parameter)?;
            let symmetry_constraint = self.add_constraint(
                format!("{label}.new_control_symmetry"),
                DocumentConstraintDefinition::SymmetricAboutLine {
                    first: source.new_control,
                    second: mirrored.new_control,
                    line: axis,
                },
            )?;
            Ok(DocumentMirroredBSplineInsertion {
                source,
                mirrored,
                symmetry_constraint,
            })
        })();
        if result.is_err() {
            let next_id = self.next_id;
            let span_cursors = self.spline_span_allocator_cursors();
            *self = before;
            self.next_id = next_id;
            self.advance_spline_span_allocators(&span_cursors);
        }
        result
    }

    fn validate_mirrored_bspline_pair(
        &self,
        source_curve: CurveId,
        mirrored_curve: CurveId,
        axis: CurveSpan,
        activity: &EffectiveActivity,
    ) -> Result<(), DocumentError> {
        self.validate_line_span(axis)?;
        if source_curve == mirrored_curve {
            return invalid("mirrored B-spline pair", "curves must be distinct");
        }
        let source = self
            .curve(source_curve)
            .ok_or_else(|| unknown("curve", source_curve.0))?;
        let mirrored = self
            .curve(mirrored_curve)
            .ok_or_else(|| unknown("curve", mirrored_curve.0))?;
        let (
            CurveDefinition::BSpline {
                form: source_form,
                degree: source_degree,
                controls: source_controls,
                knots: source_knots,
                span_ids: source_spans,
                ..
            },
            CurveDefinition::BSpline {
                form: mirrored_form,
                degree: mirrored_degree,
                controls: mirrored_controls,
                knots: mirrored_knots,
                span_ids: mirrored_spans,
                ..
            },
        ) = (&source.definition, &mirrored.definition)
        else {
            return invalid("mirrored B-spline pair", "expected two B-splines");
        };
        if source_form != mirrored_form
            || source_degree != mirrored_degree
            || source_knots != mirrored_knots
            || source_spans.len() != mirrored_spans.len()
            || source_controls.len() != mirrored_controls.len()
        {
            return invalid(
                "mirrored B-spline pair",
                "curves must have identical basis topology",
            );
        }
        let all_associated =
            source_controls
                .iter()
                .zip(mirrored_controls)
                .all(|(first, second)| {
                    self.constraints.iter().any(|constraint| {
                    activity.is_active(constraint.id)
                        && matches!(
                            constraint.definition,
                            DocumentConstraintDefinition::SymmetricAboutLine {
                                first: constraint_first,
                                second: constraint_second,
                                line,
                            } if line == axis
                                && ((constraint_first == *first && constraint_second == *second)
                                    || (constraint_first == *second && constraint_second == *first))
                        )
                })
                });
        if !all_associated {
            return invalid(
                "mirrored B-spline pair",
                "every corresponding control pair requires an active symmetry constraint",
            );
        }
        Ok(())
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
        candidate.validate_after_mutation()?;
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
        let activity = self.compute_effective_activity();
        if self.contacts.iter().any(|contact| contact.parameter == id) {
            return invalid(
                "scalar edit",
                "contact-owned scalars require an atomic contact-state edit",
            );
        }
        if self.curves.iter().any(|curve| {
            let CurveDefinition::CircularArc {
                start_angle,
                end_angle,
                ..
            } = &curve.definition
            else {
                return false;
            };
            (*start_angle == id || *end_angle == id)
                && self.constraints.iter().any(|constraint| {
                    activity.is_active(constraint.id)
                        && matches!(
                            constraint.definition,
                            DocumentConstraintDefinition::LineLineFillet { arc, .. }
                                | DocumentConstraintDefinition::CurveCurveFillet { arc, .. }
                                if arc == curve.id
                        )
                })
        }) {
            return invalid(
                "scalar edit",
                "active line-fillet endpoint angles are derived from parent contacts",
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
        candidate.validate_after_mutation()?;
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
        candidate.validate_after_mutation()?;
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
        candidate.validate_after_mutation()?;
        *self = candidate;
        Ok(())
    }

    /// Atomically replaces a rational conic's middle control and weight.
    ///
    /// Euclidean input preserves the conventional control by storing `Qh = w * P1` and requires
    /// a nonzero weight. Projective input is explicit zero-weight state and stores the supplied
    /// raw homogeneous vector unchanged. The persistent curve definition and schema remain
    /// `(weighted_middle, middle_weight)`.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing/wrong-family curve, a non-finite control, a weight outside
    /// the existing conic domain, a mode/weight mismatch, or invalid resulting conic geometry.
    pub fn set_rational_conic_control(
        &mut self,
        curve: CurveId,
        control: DocumentRationalConicControl,
    ) -> Result<(), DocumentError> {
        let (weighted_middle, weight) = match control {
            DocumentRationalConicControl::Euclidean { middle, weight } => {
                finite_pair(middle, "rational Euclidean middle")?;
                if weight == 0.0 {
                    return invalid(
                        "rational control mode",
                        "Euclidean middle control requires a nonzero weight",
                    );
                }
                let weighted_middle = rational_weighted_middle_preserving_control(middle, weight)?;
                (weighted_middle, weight)
            }
            DocumentRationalConicControl::Projective {
                weighted_middle,
                weight,
            } => {
                finite_pair(weighted_middle, "rational homogeneous middle")?;
                if weight != 0.0 {
                    return invalid(
                        "rational control mode",
                        "projective middle control requires an exact zero weight",
                    );
                }
                (weighted_middle, 0.0)
            }
        };
        let mut candidate = self.clone();
        let middle_weight = {
            let value = candidate
                .curve(curve)
                .ok_or_else(|| unknown("curve", curve.0))?;
            let CurveDefinition::RationalQuadraticConic { middle_weight, .. } = &value.definition
            else {
                return invalid(
                    "curve",
                    "rational-control edit requires a rational quadratic conic",
                );
            };
            *middle_weight
        };
        let scalar = candidate
            .scalar_mut(middle_weight)
            .ok_or_else(|| unknown("scalar", middle_weight.0))?;
        validate_scalar_value(weight, scalar.domain)?;
        scalar.value = weight;
        let CurveDefinition::RationalQuadraticConic {
            weighted_middle: current,
            ..
        } = &mut candidate
            .curve_mut(curve)
            .ok_or_else(|| unknown("curve", curve.0))?
            .definition
        else {
            unreachable!("rational curve family changed inside one atomic edit")
        };
        *current = weighted_middle;
        candidate.validate_after_mutation()?;
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
        candidate.validate_after_mutation()?;
        *self = candidate;
        Ok(())
    }

    /// Returns one line/polyline span's persistent branch direction.
    #[must_use]
    pub fn curve_branch_direction(&self, span: CurveSpan) -> Option<[f64; 2]> {
        let curve = self.curve(span.curve)?;
        match &curve.definition {
            CurveDefinition::Line {
                branch_direction, ..
            } if span.segment == 0 => Some(*branch_direction),
            CurveDefinition::Polyline {
                branch_directions, ..
            } => branch_directions.get(span.segment as usize).copied(),
            _ => None,
        }
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
        candidate.validate_after_mutation()?;
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
        candidate.validate_after_mutation()?;
        *self = candidate;
        Ok(())
    }

    /// Atomically changes both normal sides, endpoint order, and sweep of one line fillet.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing/non-fillet source or invalid resulting document state.
    pub fn set_line_line_fillet_branch(
        &mut self,
        constraint: DocumentConstraintId,
        first_side: DocumentCurveNormalSide,
        second_side: DocumentCurveNormalSide,
        endpoint_order: DocumentFilletEndpointOrder,
        sweep: DocumentArcSweep,
    ) -> Result<(), DocumentError> {
        let mut candidate = self.clone();
        let (arc, old_order) = {
            let source = candidate
                .constraints
                .iter()
                .find(|source| source.id == constraint)
                .ok_or_else(|| unknown("constraint", constraint.0))?;
            let DocumentConstraintDefinition::LineLineFillet {
                arc,
                endpoint_order,
                ..
            } = source.definition
            else {
                return invalid("constraint", "branch edit requires a line fillet");
            };
            (arc, endpoint_order)
        };
        if endpoint_order != old_order {
            let (start, end) = match &candidate
                .curve(arc)
                .ok_or_else(|| unknown("curve", arc.0))?
                .definition
            {
                CurveDefinition::CircularArc {
                    start_angle,
                    end_angle,
                    ..
                } => (*start_angle, *end_angle),
                _ => return invalid("line fillet arc", "output must remain a circular arc"),
            };
            let start_value = candidate.require_scalar(start)?.value;
            let end_value = candidate.require_scalar(end)?.value;
            candidate
                .scalar_mut(start)
                .ok_or_else(|| unknown("scalar", start.0))?
                .value = end_value;
            candidate
                .scalar_mut(end)
                .ok_or_else(|| unknown("scalar", end.0))?
                .value = start_value;
        }
        let source = candidate
            .constraints
            .iter_mut()
            .find(|source| source.id == constraint)
            .ok_or_else(|| unknown("constraint", constraint.0))?;
        let DocumentConstraintDefinition::LineLineFillet {
            first_side: current_first,
            second_side: current_second,
            endpoint_order: current_order,
            ..
        } = &mut source.definition
        else {
            return invalid("constraint", "branch edit requires a line fillet");
        };
        *current_first = first_side;
        *current_second = second_side;
        *current_order = endpoint_order;
        let output = candidate
            .curve_mut(arc)
            .ok_or_else(|| unknown("curve", arc.0))?;
        let CurveDefinition::CircularArc { sweep: current, .. } = &mut output.definition else {
            return invalid("line fillet arc", "output must remain a circular arc");
        };
        *current = sweep;
        candidate.validate_after_mutation()?;
        *self = candidate;
        Ok(())
    }

    /// Atomically changes both generic-parent sides, owned trim endpoints, arc order, and sweep.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing/non-generic association, malformed ownership, or a branch
    /// whose resulting bounded/periodic visible intervals are invalid.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub fn set_curve_curve_fillet_branch(
        &mut self,
        constraint: DocumentConstraintId,
        first_side: DocumentCurveNormalSide,
        first_trim_endpoint: DocumentFilletTrimEndpoint,
        second_side: DocumentCurveNormalSide,
        second_trim_endpoint: DocumentFilletTrimEndpoint,
        endpoint_order: DocumentFilletEndpointOrder,
        sweep: DocumentArcSweep,
    ) -> Result<(), DocumentError> {
        let mut candidate = self.clone();
        let (
            arc,
            first_contact,
            old_first_trim_endpoint,
            second_contact,
            old_second_trim_endpoint,
            old_order,
        ) = {
            let source = candidate
                .constraint(constraint)
                .ok_or_else(|| unknown("constraint", constraint.0))?;
            let DocumentConstraintDefinition::CurveCurveFillet {
                arc,
                first_contact,
                first_trim_endpoint,
                second_contact,
                second_trim_endpoint,
                endpoint_order,
                ..
            } = source.definition
            else {
                return invalid("constraint", "branch edit requires a generic curve fillet");
            };
            (
                arc,
                first_contact,
                first_trim_endpoint,
                second_contact,
                second_trim_endpoint,
                endpoint_order,
            )
        };
        for (contact, old_endpoint, new_endpoint) in [
            (first_contact, old_first_trim_endpoint, first_trim_endpoint),
            (
                second_contact,
                old_second_trim_endpoint,
                second_trim_endpoint,
            ),
        ] {
            if old_endpoint == new_endpoint {
                continue;
            }
            let slot = candidate.require_contact(contact)?.clone();
            let support = slot.curve;
            let periodic = candidate.trim_support_is_periodic(support)?;
            let retained_winding = if candidate.trim_support_allows_winding(support)? {
                slot.winding
            } else {
                0
            };
            let expected = DocumentTrimBoundary::FilletContact {
                owner: constraint,
                contact,
            };
            let view = candidate
                .trim_views
                .iter_mut()
                .find(|view| {
                    view.support == support && (view.start == expected || view.end == expected)
                })
                .ok_or_else(|| DocumentError::InvalidField {
                    field: "trim view",
                    message: "generic fillet parent has no trim view".into(),
                })?;
            let retained_fixed = match old_endpoint {
                DocumentFilletTrimEndpoint::Start if view.start == expected => view.end,
                DocumentFilletTrimEndpoint::End if view.end == expected => view.start,
                _ => {
                    return invalid(
                        "trim view ownership",
                        "generic fillet does not own its recorded parent endpoint",
                    );
                }
            };
            let DocumentTrimBoundary::Fixed(anchor) = retained_fixed else {
                return invalid(
                    "trim view ownership",
                    "a generic fillet parent requires one fixed opposite boundary",
                );
            };
            let fixed = DocumentTrimBoundary::Fixed(if periodic {
                anchor
            } else {
                DocumentTrimParameter {
                    parameter: match new_endpoint {
                        DocumentFilletTrimEndpoint::Start => 1.0,
                        DocumentFilletTrimEndpoint::End => 0.0,
                    },
                    winding: retained_winding,
                }
            });
            (view.start, view.end) = match new_endpoint {
                DocumentFilletTrimEndpoint::Start => (expected, fixed),
                DocumentFilletTrimEndpoint::End => (fixed, expected),
            };
        }
        if endpoint_order != old_order {
            candidate.swap_fillet_arc_angles(arc)?;
        }
        let source = candidate
            .constraints
            .iter_mut()
            .find(|source| source.id == constraint)
            .ok_or_else(|| unknown("constraint", constraint.0))?;
        let DocumentConstraintDefinition::CurveCurveFillet {
            first_side: current_first_side,
            first_trim_endpoint: current_first_trim_endpoint,
            second_side: current_second_side,
            second_trim_endpoint: current_second_trim_endpoint,
            endpoint_order: current_order,
            ..
        } = &mut source.definition
        else {
            return invalid("constraint", "branch edit requires a generic curve fillet");
        };
        *current_first_side = first_side;
        *current_first_trim_endpoint = first_trim_endpoint;
        *current_second_side = second_side;
        *current_second_trim_endpoint = second_trim_endpoint;
        *current_order = endpoint_order;
        let output = candidate
            .curve_mut(arc)
            .ok_or_else(|| unknown("curve", arc.0))?;
        let CurveDefinition::CircularArc { sweep: current, .. } = &mut output.definition else {
            return invalid("curve fillet arc", "output must remain a circular arc");
        };
        *current = sweep;
        candidate.validate_after_mutation()?;
        *self = candidate;
        Ok(())
    }

    fn swap_fillet_arc_angles(&mut self, arc: CurveId) -> Result<(), DocumentError> {
        let (start, end) = match &self
            .curve(arc)
            .ok_or_else(|| unknown("curve", arc.0))?
            .definition
        {
            CurveDefinition::CircularArc {
                start_angle,
                end_angle,
                ..
            } => (*start_angle, *end_angle),
            _ => return invalid("curve fillet arc", "output must remain a circular arc"),
        };
        let start_value = self.require_scalar(start)?.value;
        let end_value = self.require_scalar(end)?.value;
        self.scalar_mut(start)
            .ok_or_else(|| unknown("scalar", start.0))?
            .value = end_value;
        self.scalar_mut(end)
            .ok_or_else(|| unknown("scalar", end.0))?
            .value = start_value;
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
        let activity = self.compute_effective_activity();
        if self.constraints.iter().any(|constraint| {
            !activity.is_active(constraint.id)
                && matches!(
                    constraint.definition,
                    DocumentConstraintDefinition::CurveCurveFillet { .. }
                )
                && constraint_contacts(&constraint.definition)
                    .iter()
                    .any(|contact| requested.contains(contact))
        }) {
            return invalid(
                "contact edits",
                "inactive curve fillet contacts remain frozen with their visible intervals",
            );
        }
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
        candidate.validate_after_mutation()?;
        *self = candidate;
        Ok(())
    }

    /// Atomically replaces complete explicit branch state for one point contact or
    /// both contacts of one contact/tangency source.
    ///
    /// Contact and parameter identities are retained. A span edit may move only
    /// between semantic spans of the same owning curve; rebinding a relation to a
    /// different curve remains a topology operation rather than a branch edit.
    ///
    /// # Errors
    ///
    /// Returns an error for missing/repeated contacts, unrelated source contacts,
    /// a cross-curve rebind, an unsupported domain, or invalid resulting state.
    pub fn set_contact_branches(
        &mut self,
        edits: &[ContactBranchEdit],
    ) -> Result<(), DocumentError> {
        if edits.is_empty() || edits.len() > 2 {
            return invalid(
                "contact branch edits",
                "must contain one point contact or two relation contacts",
            );
        }
        let requested = edits
            .iter()
            .map(|edit| edit.contact)
            .collect::<BTreeSet<_>>();
        if requested.len() != edits.len() {
            return invalid("contact branch edits", "contact IDs must be distinct");
        }
        self.ordered_source_contacts(&requested.iter().copied().collect::<Vec<_>>())?;

        let mut candidate = self.clone();
        for edit in edits {
            let current = candidate
                .contacts
                .iter()
                .find(|contact| contact.id == edit.contact)
                .ok_or_else(|| unknown("contact", edit.contact.0))?
                .clone();
            if current.curve.curve != edit.curve.curve {
                return invalid(
                    "contact branch curve",
                    "branch edits must retain the owning curve",
                );
            }
            if !candidate
                .curve_contact_domains(edit.curve)?
                .contains(&edit.domain)
            {
                return invalid(
                    "contact branch domain",
                    "selected parameter domain is unsupported by the curve span",
                );
            }
            let (unit, scalar_domain) = match edit.domain {
                ContactDomain::SupportingLine => (ScalarUnit::Parameter, ScalarDomain::Finite),
                ContactDomain::Bounded { lower, upper } => (
                    ScalarUnit::Parameter,
                    ScalarDomain::Bounded { lower, upper },
                ),
                ContactDomain::Periodic { period } => {
                    (ScalarUnit::Angle, ScalarDomain::Periodic { period })
                }
            };
            {
                let contact = candidate
                    .contacts
                    .iter_mut()
                    .find(|contact| contact.id == edit.contact)
                    .ok_or_else(|| unknown("contact", edit.contact.0))?;
                contact.curve = edit.curve;
                contact.domain = edit.domain;
                contact.winding = edit.winding;
                contact.neighborhood = edit.neighborhood;
                contact.tangent_orientation = edit.tangent_orientation;
            }
            let scalar = candidate
                .scalars
                .iter_mut()
                .find(|scalar| scalar.id == current.parameter)
                .ok_or_else(|| unknown("scalar", current.parameter.0))?;
            scalar.value = edit.value;
            scalar.unit = unit;
            scalar.domain = scalar_domain;
        }
        candidate.validate_after_mutation()?;
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
        candidate.validate_after_mutation()?;
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

    /// Changes the explicit direction of an existing oriented-angle dimension.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing dimension or a non-angle dimension.
    pub fn set_oriented_angle_orientation(
        &mut self,
        id: DocumentDimensionId,
        orientation: DocumentAngleOrientation,
    ) -> Result<(), DocumentError> {
        let mut candidate = self.clone();
        let dimension = candidate
            .dimensions
            .iter_mut()
            .find(|value| value.id == id)
            .ok_or_else(|| unknown("dimension", id.0))?;
        let DocumentDimensionDefinition::OrientedAngle {
            orientation: current,
            ..
        } = &mut dimension.definition
        else {
            return invalid(
                "dimension",
                "orientation edit requires an oriented-angle dimension",
            );
        };
        *current = orientation;
        candidate.validate_after_mutation()?;
        *self = candidate;
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
        self.set_element_user_suppressed(DocumentElementId::Source(source), suppressed)
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
                .then(|| {
                    candidate
                        .user_inactive_elements
                        .remove(&DocumentElementId::Point(id));
                })
                .ok_or_else(|| unknown("point", id.0))?,
            DocumentObjectId::Scalar(id) => retain_remove(&mut candidate.scalars, |v| v.id == id)
                .then(|| {
                    candidate
                        .user_inactive_elements
                        .remove(&DocumentElementId::Scalar(id));
                })
                .ok_or_else(|| unknown("scalar", id.0))?,
            DocumentObjectId::Curve(id) => {
                let owned_scalars = curve_owned_scalars(
                    &candidate
                        .curve(id)
                        .ok_or_else(|| unknown("curve", id.0))?
                        .definition,
                );
                retain_remove(&mut candidate.curves, |value| value.id == id);
                candidate.geometry_roles.remove(&id);
                candidate
                    .user_inactive_elements
                    .remove(&DocumentElementId::Curve(id));
                candidate
                    .scalars
                    .retain(|value| !owned_scalars.contains(&value.id));
                candidate.trim_views.retain(|view| view.support.curve != id);
            }
            DocumentObjectId::Contact(id) => retain_remove(&mut candidate.contacts, |v| v.id == id)
                .then(|| {
                    candidate
                        .user_inactive_elements
                        .remove(&DocumentElementId::Contact(id));
                })
                .ok_or_else(|| unknown("contact", id.0))?,
            DocumentObjectId::Constraint(id) => {
                candidate.freeze_generic_fillet_boundaries(id)?;
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
            DocumentObjectId::Parameter(id) => {
                retain_remove(&mut candidate.parameters, |parameter| parameter.id == id)
                    .then(|| {
                        candidate
                            .user_inactive_elements
                            .remove(&DocumentElementId::Parameter(id));
                    })
                    .ok_or_else(|| unknown("parameter", id.0))?;
            }
            DocumentObjectId::ExternalBinding(id) => {
                retain_remove(&mut candidate.external_bindings, |binding| binding.id == id)
                    .then(|| {
                        candidate
                            .user_inactive_elements
                            .remove(&DocumentElementId::ExternalBinding(id));
                    })
                    .ok_or_else(|| unknown("external binding", id.0))?;
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

    fn freeze_generic_fillet_boundaries(
        &mut self,
        constraint: DocumentConstraintId,
    ) -> Result<(), DocumentError> {
        self.constraint(constraint)
            .ok_or_else(|| unknown("constraint", constraint.0))?;
        let owned = self
            .trim_views
            .iter()
            .flat_map(|view| [view.start, view.end])
            .filter_map(|boundary| match boundary {
                DocumentTrimBoundary::FilletContact { owner, contact }
                | DocumentTrimBoundary::ConstraintContact { owner, contact }
                    if owner == constraint =>
                {
                    Some(contact)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        for contact in owned {
            let slot = self.require_contact(contact)?.clone();
            let accepted = DocumentTrimBoundary::Fixed(DocumentTrimParameter {
                parameter: self.require_scalar(slot.parameter)?.value,
                winding: slot.winding,
            });
            let view = self
                .trim_views
                .iter_mut()
                .find(|view| {
                    let owns = |boundary| {
                        matches!(
                            boundary,
                            DocumentTrimBoundary::FilletContact { owner, contact: owned }
                                | DocumentTrimBoundary::ConstraintContact {
                                    owner,
                                    contact: owned,
                                } if owner == constraint && owned == contact
                        )
                    };
                    view.support == slot.curve && (owns(view.start) || owns(view.end))
                })
                .ok_or_else(|| DocumentError::InvalidField {
                    field: "trim view",
                    message: "constraint has no owned trim view".into(),
                })?;
            let owns = |boundary| {
                matches!(
                    boundary,
                    DocumentTrimBoundary::FilletContact { owner, contact: owned }
                        | DocumentTrimBoundary::ConstraintContact {
                            owner,
                            contact: owned,
                        } if owner == constraint && owned == contact
                )
            };
            if owns(view.start) {
                view.start = accepted;
            } else if owns(view.end) {
                view.end = accepted;
            } else {
                return invalid(
                    "trim view ownership",
                    "constraint does not own its recorded boundary",
                );
            }
        }
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
        candidate.validate_after_mutation()?;
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
        if let Some(arc) = self.constraints.iter().find_map(|constraint| {
            let (DocumentConstraintDefinition::LineLineFillet { arc, .. }
            | DocumentConstraintDefinition::CurveCurveFillet { arc, .. }) = constraint.definition
            else {
                return None;
            };
            removal
                .contains(&DocumentObjectId::Curve(arc))
                .then_some(arc)
        }) {
            return Err(DocumentError::ObjectInUse(arc.0));
        }

        let mut removal = removal.into_iter().collect::<Vec<_>>();
        removal.sort_by_key(|object| match object {
            DocumentObjectId::Constraint(_)
            | DocumentObjectId::Dimension(_)
            | DocumentObjectId::Parameter(_)
            | DocumentObjectId::ExternalBinding(_) => 0,
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
        candidate.validate_after_mutation()?;
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
        let completed = self.validate_with_controller(None)?;
        debug_assert!(completed, "uncontrolled validation cannot be interrupted");
        Ok(())
    }

    pub(crate) fn validate_after_mutation(&self) -> Result<(), DocumentError> {
        if self.mutation_validation_deferred {
            Ok(())
        } else {
            self.validate()
        }
    }

    pub(crate) fn defer_mutation_validation(&mut self) {
        self.mutation_validation_deferred = true;
    }

    pub(crate) fn resume_mutation_validation(&mut self) {
        self.mutation_validation_deferred = false;
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn validate_with_controller(
        &self,
        mut controller: Option<&mut OperationController>,
    ) -> Result<bool, DocumentError> {
        if !charge_document_item(
            &mut controller,
            OperationWorkCounter::DocumentValidationItems,
            OperationCheckpoint::DocumentValidation,
        ) {
            return Ok(false);
        }
        if self.version != SKETCH_DOCUMENT_VERSION {
            return Err(DocumentError::UnsupportedVersion {
                actual: self.version,
                expected: SKETCH_DOCUMENT_VERSION,
            });
        }
        finite_positive(self.model_scale, "model_scale")?;
        let activity = self.compute_effective_activity();
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
            + self.trim_views.len()
            + self.constraints.len() * 2
            + self.dimensions.len() * 2
            + self.parameters.len()
            + self.parameter_bindings.len()
            + self.parameter_outputs.len()
            + self.external_bindings.len()
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
            if !charge_document_item(
                &mut controller,
                OperationWorkCounter::DocumentValidationItems,
                OperationCheckpoint::DocumentValidation,
            ) {
                return Ok(false);
            }
            insert_unique(&mut ids, point.id.0)?;
            validate_label(&point.label, "point label")?;
            finite_pair(point.position, "point position")?;
        }
        for scalar in &self.scalars {
            if !charge_document_item(
                &mut controller,
                OperationWorkCounter::DocumentValidationItems,
                OperationCheckpoint::DocumentValidation,
            ) {
                return Ok(false);
            }
            insert_unique(&mut ids, scalar.id.0)?;
            validate_label(&scalar.label, "scalar label")?;
            validate_scalar_value(scalar.value, scalar.domain)?;
        }
        let mut used_scalars = BTreeSet::new();
        for curve in &self.curves {
            if !charge_document_item(
                &mut controller,
                OperationWorkCounter::DocumentValidationItems,
                OperationCheckpoint::DocumentValidation,
            ) || !charge_document_item(
                &mut controller,
                OperationWorkCounter::DocumentDependencyItems,
                OperationCheckpoint::DocumentDependency,
            ) {
                return Ok(false);
            }
            insert_unique(&mut ids, curve.id.0)?;
            validate_label(&curve.label, "curve label")?;
            self.validate_curve_definition(curve.id, &curve.definition, &activity)?;
            for scalar in curve_scalars(&curve.definition) {
                claim_scalar(&mut used_scalars, scalar)?;
            }
        }
        let mut contact_scalars = BTreeSet::new();
        for contact in &self.contacts {
            if !charge_document_item(
                &mut controller,
                OperationWorkCounter::DocumentValidationItems,
                OperationCheckpoint::DocumentValidation,
            ) || !charge_document_item(
                &mut controller,
                OperationWorkCounter::DocumentDependencyItems,
                OperationCheckpoint::DocumentDependency,
            ) {
                return Ok(false);
            }
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
        let mut prior_trim_end = BTreeMap::<CurveSpan, f64>::new();
        for view in &self.trim_views {
            if !charge_document_item(
                &mut controller,
                OperationWorkCounter::DocumentValidationItems,
                OperationCheckpoint::DocumentValidation,
            ) || !charge_document_item(
                &mut controller,
                OperationWorkCounter::DocumentDependencyItems,
                OperationCheckpoint::DocumentDependency,
            ) {
                return Ok(false);
            }
            let interval = self.resolve_trim_view(view)?;
            if let Some(prior_end) = prior_trim_end.insert(view.support, interval.end)
                && prior_end > interval.start
            {
                return invalid(
                    "trim view intervals",
                    "visible intervals on one support must be traversal-ordered and non-overlapping",
                );
            }
        }
        let mut sources = BTreeSet::new();
        let mut used_contacts = BTreeSet::new();
        let mut fillet_arcs = BTreeSet::new();
        for constraint in &self.constraints {
            if !charge_document_item(
                &mut controller,
                OperationWorkCounter::DocumentValidationItems,
                OperationCheckpoint::DocumentValidation,
            ) || !charge_document_item(
                &mut controller,
                OperationWorkCounter::DocumentDependencyItems,
                OperationCheckpoint::DocumentDependency,
            ) {
                return Ok(false);
            }
            insert_unique(&mut ids, constraint.id.0)?;
            insert_unique(&mut ids, constraint.source_id.0)?;
            sources.insert(constraint.source_id);
            validate_label(&constraint.label, "constraint label")?;
            self.validate_constraint_definition(&constraint.definition)?;
            let fillet_arc = match constraint.definition {
                DocumentConstraintDefinition::LineLineFillet { arc, .. }
                | DocumentConstraintDefinition::CurveCurveFillet { arc, .. } => Some(arc),
                _ => None,
            };
            if let Some(arc) = fillet_arc
                && !fillet_arcs.insert(arc)
            {
                return invalid(
                    "fillet arc",
                    "one output arc may belong to only one fillet association",
                );
            }
            for contact in constraint_contacts(&constraint.definition) {
                if !used_contacts.insert(contact) {
                    return invalid(
                        "constraint contact",
                        "a contact slot may belong to only one constraint source",
                    );
                }
            }
        }
        for constraint in &self.constraints {
            if !charge_document_item(
                &mut controller,
                OperationWorkCounter::DocumentDependencyItems,
                OperationCheckpoint::DocumentDependency,
            ) {
                return Ok(false);
            }
            if let DocumentConstraintDefinition::CurveCurveFillet {
                first_contact,
                first_trim_endpoint,
                second_contact,
                second_trim_endpoint,
                ..
            } = constraint.definition
            {
                self.validate_owned_trim_boundary(
                    constraint.id,
                    first_contact,
                    first_trim_endpoint,
                )?;
                self.validate_owned_trim_boundary(
                    constraint.id,
                    second_contact,
                    second_trim_endpoint,
                )?;
            }
        }
        for constraint in self
            .constraints
            .iter()
            .filter(|constraint| activity.is_active(constraint.id))
        {
            if !charge_document_item(
                &mut controller,
                OperationWorkCounter::DocumentDependencyItems,
                OperationCheckpoint::DocumentDependency,
            ) {
                return Ok(false);
            }
            for contact in constraint_contacts(&constraint.definition) {
                let slot = self.require_contact(contact)?;
                if self.trim_views_for_span(slot.curve).next().is_none() {
                    continue;
                }
                let intervals = self.visible_intervals(slot.curve)?;
                let value = self.resolve_fixed_trim_parameter(
                    slot.curve,
                    DocumentTrimParameter {
                        parameter: self.require_scalar(slot.parameter)?.value,
                        winding: slot.winding,
                    },
                )?;
                let fillet_boundary = DocumentTrimBoundary::FilletContact {
                    owner: constraint.id,
                    contact,
                };
                let constraint_boundary = DocumentTrimBoundary::ConstraintContact {
                    owner: constraint.id,
                    contact,
                };
                let owned_interval = intervals.iter().find(|interval| {
                    interval.start_boundary == fillet_boundary
                        || interval.end_boundary == fillet_boundary
                        || interval.start_boundary == constraint_boundary
                        || interval.end_boundary == constraint_boundary
                });
                if let Some(interval) = owned_interval {
                    if value.to_bits() != interval.start.to_bits()
                        && value.to_bits() != interval.end.to_bits()
                    {
                        return invalid(
                            "trim contact visibility",
                            "owned contact must resolve to its visible boundary",
                        );
                    }
                } else if !intervals
                    .iter()
                    .any(|interval| interval.start < value && value < interval.end)
                {
                    return invalid(
                        "trim contact visibility",
                        "executable non-owner contact must lie inside a visible interval",
                    );
                }
            }
        }
        for dimension in &self.dimensions {
            if !charge_document_item(
                &mut controller,
                OperationWorkCounter::DocumentValidationItems,
                OperationCheckpoint::DocumentValidation,
            ) || !charge_document_item(
                &mut controller,
                OperationWorkCounter::DocumentDependencyItems,
                OperationCheckpoint::DocumentDependency,
            ) {
                return Ok(false);
            }
            insert_unique(&mut ids, dimension.id.0)?;
            insert_unique(&mut ids, dimension.source_id.0)?;
            sources.insert(dimension.source_id);
            validate_label(&dimension.label, "dimension label")?;
            self.validate_dimension_definition(&dimension.definition, dimension.mode)?;
            claim_scalar(&mut used_scalars, dimension_target(&dimension.definition))?;
        }
        if self.parameters.len() > MAX_DOCUMENT_PARAMETERS {
            return Err(DocumentError::ResourceLimit {
                resource: "parameters",
                actual: self.parameters.len(),
                limit: MAX_DOCUMENT_PARAMETERS,
            });
        }
        for parameter in &self.parameters {
            insert_unique(&mut ids, parameter.id.0)?;
            validate_label(&parameter.label, "parameter label")?;
        }
        if self.external_bindings.len() > MAX_EXTERNAL_BINDINGS {
            return Err(DocumentError::ResourceLimit {
                resource: "external bindings",
                actual: self.external_bindings.len(),
                limit: MAX_EXTERNAL_BINDINGS,
            });
        }
        for binding in &self.external_bindings {
            insert_unique(&mut ids, binding.id.0)?;
            validate_label(&binding.label, "external binding label")?;
            validate_external_binding_shape(binding.expected_kind, binding.expected_topology)?;
        }
        let mut binding_pairs = BTreeSet::new();
        let mut binding_targets = BTreeSet::new();
        let mut input_parameters = BTreeSet::new();
        for binding in &self.parameter_bindings {
            let target_key = canonical_parameter_target_key(binding.target);
            if !binding_pairs.insert((binding.parameter, target_key)) {
                return invalid("parameter binding", "duplicate association");
            }
            if !binding_targets.insert(target_key) {
                return invalid("parameter binding", "one target may have only one supplier");
            }
            let parameter = self
                .parameter(binding.parameter)
                .ok_or_else(|| unknown("parameter", binding.parameter.0))?;
            input_parameters.insert(binding.parameter);
            match binding.target {
                DocumentParameterTarget::DrivingDimension(id) => {
                    let dimension = self
                        .dimension(id)
                        .ok_or_else(|| unknown("parameter dimension target", id.0))?;
                    if dimension.mode != DocumentDimensionMode::Driving {
                        return invalid(
                            "parameter binding",
                            "only driving dimensions may consume host inputs",
                        );
                    }
                    if parameter.kind != dimension_parameter_kind(&dimension.definition) {
                        return invalid(
                            "parameter binding",
                            "parameter kind is incompatible with the dimension target",
                        );
                    }
                }
                DocumentParameterTarget::DimensionlessFixedScalar(property) => {
                    if parameter.kind != DocumentParameterKind::Dimensionless {
                        return invalid(
                            "parameter binding",
                            "dimensionless scalar targets require a dimensionless parameter",
                        );
                    }
                    self.validate_dimensionless_parameter_property(property)?;
                }
                DocumentParameterTarget::Activation(element) => {
                    if parameter.kind != DocumentParameterKind::Activation {
                        return invalid(
                            "parameter binding",
                            "activation targets require an activation parameter",
                        );
                    }
                    if !self.contains_element(element) {
                        return Err(unknown(element.kind(), element.persistent_id()));
                    }
                    if matches!(
                        element,
                        DocumentElementId::Document(_) | DocumentElementId::Parameter(_)
                    ) {
                        return invalid(
                            "parameter binding",
                            "document and parameter declarations are not activation targets",
                        );
                    }
                }
            }
        }
        let mut outputs = BTreeSet::new();
        let mut output_parameters = BTreeSet::new();
        for output in &self.parameter_outputs {
            if !outputs.insert(*output) {
                return invalid("parameter output", "duplicate declaration");
            }
            let parameter = self
                .parameter(output.parameter)
                .ok_or_else(|| unknown("parameter", output.parameter.0))?;
            output_parameters.insert(output.parameter);
            let dimension = self
                .dimension(output.dimension)
                .ok_or_else(|| unknown("parameter output dimension", output.dimension.0))?;
            if dimension.mode != DocumentDimensionMode::Reference {
                return invalid(
                    "parameter output",
                    "only reference dimensions may produce output proposals",
                );
            }
            if parameter.kind != dimension_parameter_kind(&dimension.definition) {
                return invalid(
                    "parameter output",
                    "parameter kind is incompatible with the reference dimension",
                );
            }
        }
        if input_parameters
            .intersection(&output_parameters)
            .next()
            .is_some()
        {
            return invalid(
                "parameter ownership",
                "input and output parameter sets must be disjoint",
            );
        }
        let mut ordered = BTreeSet::new();
        for source in &self.source_order {
            if !charge_document_item(
                &mut controller,
                OperationWorkCounter::DocumentValidationItems,
                OperationCheckpoint::DocumentValidation,
            ) {
                return Ok(false);
            }
            ordered.insert(*source);
        }
        if ordered.len() != self.source_order.len() || ordered != sources {
            return invalid("source_order", "must contain every source exactly once");
        }
        for (curve, role) in &self.geometry_roles {
            if self.curve(*curve).is_none() {
                return Err(unknown("curve role", curve.0));
            }
            if *role == GeometryRole::Profile {
                return invalid("geometry role", "default profile roles must be implicit");
            }
        }
        for element in &self.user_inactive_elements {
            if !self.contains_element(*element) {
                return Err(unknown(element.kind(), element.persistent_id()));
            }
            if matches!(
                element,
                DocumentElementId::Constraint(_)
                    | DocumentElementId::Dimension(_)
                    | DocumentElementId::Source(_)
            ) {
                return invalid(
                    "user activation",
                    "source-owned suppression must use the v1-v4 suppressed field",
                );
            }
        }
        if let Some(activation) = &self.host_activation {
            let canonical = HostConfigurationActivation::from_digest(
                activation.revision,
                activation.digest,
                activation.overrides.clone(),
            )?;
            for entry in canonical.overrides() {
                if !self.contains_element(entry.element()) {
                    return Err(unknown(
                        entry.element().kind(),
                        entry.element().persistent_id(),
                    ));
                }
            }
        }
        let maximum = ids.iter().map(|id| id.as_u128()).max().unwrap_or(0);
        if self.next_id.as_u128() == 0 || self.next_id.as_u128() <= maximum {
            return invalid(
                "next_id",
                "must be greater than every allocated persistent ID",
            );
        }
        Ok(true)
    }

    /// Serializes a normalized deterministic JSON representation.
    ///
    /// # Errors
    ///
    /// Returns a validation or JSON serialization error.
    pub fn to_canonical_json(&self) -> Result<String, DocumentError> {
        self.validate()?;
        if self.dimensions.iter().any(|dimension| {
            matches!(
                dimension.definition,
                DocumentDimensionDefinition::ProfileOffset { .. }
            )
        }) {
            return Err(DocumentError::UnsupportedM80State);
        }
        if self
            .constraints
            .iter()
            .any(|constraint| is_datum_constraint(&constraint.definition))
        {
            return Err(DocumentError::UnsupportedM74State);
        }
        if self
            .constraints
            .iter()
            .any(|constraint| is_retained_planar_constraint(&constraint.definition))
        {
            return Err(DocumentError::UnsupportedM71State);
        }
        if !self.geometry_roles.is_empty()
            || !self.user_inactive_elements.is_empty()
            || self.host_activation.is_some()
        {
            return Err(DocumentError::UnsupportedM41State);
        }
        if !self.parameters.is_empty()
            || !self.parameter_bindings.is_empty()
            || !self.parameter_outputs.is_empty()
        {
            return Err(DocumentError::UnsupportedM42State);
        }
        if !self.external_bindings.is_empty()
            || self.constraints.iter().any(|constraint| {
                matches!(
                    constraint.definition,
                    DocumentConstraintDefinition::ExternalPointCoincident { .. }
                        | DocumentConstraintDefinition::ExternalLineCollinear { .. }
                )
            })
        {
            return Err(DocumentError::UnsupportedM43State);
        }
        if !self.v4_trim_state_supported() {
            return Err(DocumentError::UnsupportedM58State);
        }
        let mut canonical = self.clone();
        canonical.canonicalize();
        Ok(serde_json::to_string(&SketchDocumentV4::try_from(
            &canonical,
        )?)?)
    }

    /// Explicitly unsupported draft-v5 codec for pre-M62 M41 state.
    #[doc(hidden)]
    pub fn to_draft_v5_json(&self) -> Result<String, DocumentError> {
        self.validate()?;
        let mut canonical = self.clone();
        canonical.canonicalize();
        let geometry_roles = canonical
            .geometry_roles
            .iter()
            .map(|(curve, role)| DraftGeometryRole {
                curve: *curve,
                role: *role,
            })
            .collect();
        let mut constraints = Vec::with_capacity(canonical.constraints.len());
        let mut retained_planar_constraints = Vec::new();
        for constraint in &canonical.constraints {
            if let Some(retained) = DraftRetainedPlanarConstraint::from_constraint(constraint) {
                retained_planar_constraints.push(retained);
            } else {
                constraints.push(DocumentConstraintV4::try_from(constraint)?);
            }
        }
        let mut dimensions = Vec::with_capacity(canonical.dimensions.len());
        let mut profile_offset_dimensions = Vec::new();
        for dimension in &canonical.dimensions {
            if matches!(
                dimension.definition,
                DocumentDimensionDefinition::ProfileOffset { .. }
            ) {
                profile_offset_dimensions
                    .push(DraftProfileOffsetDimension::try_from_dimension(dimension)?);
            } else {
                dimensions.push(DocumentDimensionV4::try_from(dimension)?);
            }
        }
        let document = SketchDocumentV4::with_sources(&canonical, constraints, dimensions);
        let draft = SketchDocumentDraftV5 {
            version: 5,
            document,
            geometry_roles,
            user_inactive_elements: canonical.user_inactive_elements.iter().copied().collect(),
            host_activation: canonical.host_activation,
            parameters: canonical.parameters,
            parameter_bindings: canonical.parameter_bindings,
            parameter_outputs: canonical.parameter_outputs,
            external_bindings: canonical.external_bindings,
            retained_planar_constraints,
            profile_offset_dimensions,
        };
        Ok(serde_json::to_string(&draft)?)
    }

    /// Restores the explicitly unsupported pre-M62 draft-v5 representation.
    #[doc(hidden)]
    #[allow(
        clippy::too_many_lines,
        reason = "one closed draft decoder keeps compatibility validation and side-table merging atomic"
    )]
    pub fn from_draft_v5_json(json: &str) -> Result<Self, DocumentError> {
        if json.len() > MAX_DOCUMENT_JSON_BYTES {
            return Err(DocumentError::ResourceLimit {
                resource: "JSON bytes",
                actual: json.len(),
                limit: MAX_DOCUMENT_JSON_BYTES,
            });
        }
        let draft: SketchDocumentDraftV5 = serde_json::from_str(json)?;
        if draft.version != 5 {
            return Err(DocumentError::UnsupportedVersion {
                actual: draft.version,
                expected: 5,
            });
        }
        if draft.document.version != SKETCH_DOCUMENT_VERSION {
            return Err(DocumentError::UnsupportedVersion {
                actual: draft.document.version,
                expected: SKETCH_DOCUMENT_VERSION,
            });
        }
        let mut document = Self::from(draft.document);
        let side_constraint_count = draft.retained_planar_constraints.len();
        if document
            .constraints
            .len()
            .saturating_add(side_constraint_count)
            > MAX_DOCUMENT_OBJECTS / 2
        {
            return Err(DocumentError::ResourceLimit {
                resource: "retained planar constraints",
                actual: side_constraint_count,
                limit: MAX_DOCUMENT_OBJECTS / 2,
            });
        }
        let embedded_source_order = document
            .source_order
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut side_ids = BTreeSet::new();
        let mut side_sources = BTreeSet::new();
        for constraint in draft.retained_planar_constraints {
            if !side_ids.insert(constraint.id) || !side_sources.insert(constraint.source_id) {
                return invalid(
                    "retained planar constraints",
                    "constraint and source identities must be unique",
                );
            }
            if !embedded_source_order.contains(&constraint.source_id) {
                return invalid(
                    "retained planar constraints",
                    "every side constraint source must occur in the embedded source order",
                );
            }
            document
                .constraints
                .push(DocumentConstraint::from(constraint));
        }
        let side_dimension_count = draft.profile_offset_dimensions.len();
        if document
            .dimensions
            .len()
            .saturating_add(side_dimension_count)
            > MAX_DOCUMENT_OBJECTS / 2
        {
            return Err(DocumentError::ResourceLimit {
                resource: "profile offset dimensions",
                actual: side_dimension_count,
                limit: MAX_DOCUMENT_OBJECTS / 2,
            });
        }
        let mut side_dimension_ids = BTreeSet::new();
        let mut side_dimension_sources = BTreeSet::new();
        for dimension in draft.profile_offset_dimensions {
            if !side_dimension_ids.insert(dimension.id)
                || !side_dimension_sources.insert(dimension.source_id)
            {
                return invalid(
                    "profile offset dimensions",
                    "dimension and source identities must be unique",
                );
            }
            if !embedded_source_order.contains(&dimension.source_id) {
                return invalid(
                    "profile offset dimensions",
                    "every side dimension source must occur in the embedded source order",
                );
            }
            document.dimensions.push(DocumentDimension::from(dimension));
        }
        for role in draft.geometry_roles {
            if role.role == GeometryRole::Profile
                || document
                    .geometry_roles
                    .insert(role.curve, role.role)
                    .is_some()
            {
                return invalid("draft geometry roles", "must be unique non-default roles");
            }
        }
        for element in draft.user_inactive_elements {
            if !document.user_inactive_elements.insert(element) {
                return Err(DocumentError::DuplicateActivationElement(element));
            }
        }
        document.host_activation = match draft.host_activation {
            Some(activation) => Some(HostConfigurationActivation::from_digest(
                activation.revision,
                activation.digest,
                activation.overrides,
            )?),
            None => None,
        };
        document.parameters = draft.parameters;
        document.parameter_bindings = draft.parameter_bindings;
        document.parameter_outputs = draft.parameter_outputs;
        document.external_bindings = draft.external_bindings;
        document.validate()?;
        document.canonicalize();
        Ok(document)
    }

    /// Parses and strictly validates a versioned JSON document.
    ///
    /// # Errors
    ///
    /// Returns a JSON, schema, resource, value, reference, or ordering error.
    pub fn from_json(json: &str) -> Result<Self, DocumentError> {
        let mut document = Self::parse_json(json)?;
        document.validate()?;
        document.canonicalize();
        Ok(document)
    }

    pub(crate) fn from_json_with_controller(
        json: &str,
        controller: &mut OperationController,
    ) -> Result<Option<Self>, DocumentError> {
        if controller
            .checkpoint(OperationCheckpoint::DocumentValidation)
            .is_err()
        {
            return Ok(None);
        }
        let mut document = Self::parse_json(json)?;
        if !document.validate_with_controller(Some(controller))? {
            return Ok(None);
        }
        document.canonicalize();
        Ok(Some(document))
    }

    fn parse_json(json: &str) -> Result<Self, DocumentError> {
        if json.len() > MAX_DOCUMENT_JSON_BYTES {
            return Err(DocumentError::ResourceLimit {
                resource: "JSON bytes",
                actual: json.len(),
                limit: MAX_DOCUMENT_JSON_BYTES,
            });
        }
        let header: DocumentHeader = serde_json::from_str(json)?;
        let document = match header.version {
            1 => {
                let wire = serde_json::from_str::<SketchDocumentV1>(json)?;
                validate_legacy_contact_language(&wire.contacts)?;
                Self::from(wire)
            }
            2 => {
                let wire = serde_json::from_str::<SketchDocumentV2>(json)?;
                validate_legacy_contact_language(&wire.contacts)?;
                Self::from(wire)
            }
            3 => {
                let wire = serde_json::from_str::<SketchDocumentV3>(json)?;
                validate_legacy_contact_language(&wire.contacts)?;
                Self::from(wire)
            }
            4 => {
                let document = Self::from(serde_json::from_str::<SketchDocumentV4>(json)?);
                if !document.v4_trim_state_supported() {
                    return Err(DocumentError::UnsupportedM58State);
                }
                document
            }
            actual => {
                return Err(DocumentError::UnsupportedVersion {
                    actual,
                    expected: SKETCH_DOCUMENT_VERSION,
                });
            }
        };
        Ok(document)
    }

    fn canonicalize(&mut self) {
        self.points.sort_by_key(|value| value.id);
        self.scalars.sort_by_key(|value| value.id);
        self.curves.sort_by_key(|value| value.id);
        self.contacts.sort_by_key(|value| value.id);
        self.trim_views.sort_by_key(|value| value.support);
        self.constraints.sort_by_key(|value| value.id);
        self.dimensions.sort_by_key(|value| value.id);
        self.parameters.sort_by_key(|value| value.id);
        self.parameter_bindings.sort_by_key(|binding| {
            (
                binding.parameter,
                canonical_parameter_target_key(binding.target),
            )
        });
        self.parameter_outputs.sort();
        self.external_bindings.sort_by_key(|binding| binding.id);
    }

    fn v4_trim_state_supported(&self) -> bool {
        let mut supports = BTreeSet::new();
        self.trim_views.iter().all(|view| {
            supports.insert(view.support)
                && matches!(
                    view.start,
                    DocumentTrimBoundary::Fixed(_) | DocumentTrimBoundary::FilletContact { .. }
                )
                && matches!(
                    view.end,
                    DocumentTrimBoundary::Fixed(_) | DocumentTrimBoundary::FilletContact { .. }
                )
        })
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

    /// Captures every persistent identity allocator cursor owned by this sketch.
    ///
    /// The returned DTO keeps allocator fields private while allowing hosts to
    /// serialize, deserialize and merge validated checkpoint state.
    #[must_use]
    pub fn persistent_identity_high_water(&self) -> SketchPersistentIdentityHighWater {
        SketchPersistentIdentityHighWater {
            document: self.id,
            next_id: self.next_id,
            spline_span_cursors: self.spline_span_allocator_cursors(),
        }
    }

    /// Advances this document above allocator cursors retained by the same sketch.
    ///
    /// Existing persistent objects and curve topology are unchanged. Curves absent
    /// from this particular historical graph simply cannot consume their retained
    /// curve-local span cursor until a later restore reintroduces that curve.
    ///
    /// # Errors
    ///
    /// Rejects high-water metadata from a different persistent sketch namespace.
    pub fn retain_persistent_identity_high_water(
        &mut self,
        high_water: &SketchPersistentIdentityHighWater,
    ) -> Result<(), DocumentError> {
        if self.id != high_water.document {
            return invalid(
                "persistent identity high-water",
                "cursor belongs to a different sketch document",
            );
        }
        high_water.validate()?;
        self.advance_allocator(high_water.next_id);
        self.advance_spline_span_allocators(&high_water.spline_span_cursors);
        Ok(())
    }

    pub(crate) fn allocate_semantic_catalog_id(
        &mut self,
    ) -> Result<DocumentSourceId, DocumentError> {
        let id = DocumentSourceId(self.allocate_id()?);
        self.semantic_source_reservations.insert(id, id);
        Ok(id)
    }

    pub(crate) fn allocate_semantic_source_id(
        &mut self,
        catalog: DocumentSourceId,
    ) -> Result<DocumentSourceId, DocumentError> {
        if self.semantic_source_reservations.get(&catalog) != Some(&catalog) {
            return invalid(
                "semantic catalog identity",
                "catalog is not reserved by this document",
            );
        }
        let id = DocumentSourceId(self.allocate_id()?);
        self.semantic_source_reservations.insert(id, catalog);
        Ok(id)
    }

    pub(crate) fn semantic_reservation_owner(
        &self,
        id: DocumentSourceId,
    ) -> Option<DocumentSourceId> {
        self.semantic_source_reservations.get(&id).copied()
    }

    pub(crate) fn register_semantic_catalog(
        &mut self,
        catalog: DocumentSourceId,
        sources: &[DocumentSourceId],
    ) -> Result<(), DocumentError> {
        let mut ids = Vec::with_capacity(sources.len() + 1);
        ids.push(catalog);
        ids.extend_from_slice(sources);
        let mut unique = BTreeSet::new();
        for id in &ids {
            if !unique.insert(*id) {
                return Err(DocumentError::DuplicateId(id.0));
            }
            if id.0 >= self.next_id || self.element(id.0).is_some() {
                return invalid(
                    "semantic source identity",
                    "identity is not a reserved, non-element document ID",
                );
            }
            if self.semantic_source_reservations.contains_key(id) {
                return invalid(
                    "semantic source identity",
                    "identity is already owned by a loaded semantic catalog",
                );
            }
        }
        self.semantic_source_reservations.insert(catalog, catalog);
        for source in sources {
            self.semantic_source_reservations.insert(*source, catalog);
        }
        Ok(())
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

    pub(crate) fn curve_branch_is_enforced_with_activity(
        &self,
        span: CurveSpan,
        activity: &EffectiveActivity,
    ) -> bool {
        let has_axis_constraint = self
            .constraints
            .iter()
            .filter(|constraint| activity.is_active(constraint.id))
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
                activity.is_active(dimension.id) && dimension.mode == DocumentDimensionMode::Driving
            })
            .any(|dimension| {
                matches!(
                    dimension.definition,
                    DocumentDimensionDefinition::CurveLength { curve, .. } if curve == span
                )
            });
        let has_sided_fillet = self
            .constraints
            .iter()
            .filter(|constraint| activity.is_active(constraint.id))
            .any(|constraint| {
                let (DocumentConstraintDefinition::LineLineFillet {
                    first_contact,
                    second_contact,
                    ..
                }
                | DocumentConstraintDefinition::CurveCurveFillet {
                    first_contact,
                    second_contact,
                    ..
                }) = constraint.definition
                else {
                    return false;
                };
                [first_contact, second_contact]
                    .into_iter()
                    .filter_map(|contact| self.contact(contact))
                    .any(|contact| contact.curve == span)
            });
        has_sided_fillet || (has_axis_constraint && has_driving_length)
    }

    fn compute_effective_activity(&self) -> EffectiveActivity {
        self.compute_effective_activity_with_input_overlays(&BTreeSet::new(), &BTreeSet::new())
    }

    fn compute_effective_activity_with_input_overlays(
        &self,
        parameter_inactive: &BTreeSet<DocumentElementId>,
        unavailable_external: &BTreeSet<DocumentElementId>,
    ) -> EffectiveActivity {
        let mut elements = self.canonical_elements();
        let mut reasons = BTreeMap::<DocumentElementId, InactivityReason>::new();
        for element in &elements {
            if let Some(reason) = self.direct_inactivity_reason(*element) {
                reasons.insert(*element, reason);
            } else if parameter_inactive.contains(element) {
                reasons.insert(*element, InactivityReason::HostConfigurationInactive);
            } else if unavailable_external.contains(element) {
                reasons.insert(*element, InactivityReason::UnavailableExternalReference);
            }
        }
        if !reasons.is_empty() {
            loop {
                let mut changed = false;
                for element in &elements {
                    if reasons.contains_key(element) {
                        continue;
                    }
                    if let Some(dependency) = self
                        .direct_dependencies(*element)
                        .into_iter()
                        .find(|dependency| reasons.contains_key(dependency))
                    {
                        reasons.insert(
                            *element,
                            InactivityReason::UnavailableDependency { dependency },
                        );
                        changed = true;
                    }
                }
                if !changed {
                    break;
                }
            }
        }
        let (activation_revision, activation_digest) = self
            .host_activation
            .as_ref()
            .map_or((0, ActivationDigest::default()), |activation| {
                (activation.revision(), activation.digest())
            });
        let entries = elements
            .drain(..)
            .map(|element| DocumentElementActivity {
                element,
                reason: reasons.get(&element).copied(),
            })
            .collect();
        EffectiveActivity {
            activation_revision,
            activation_digest,
            elements: entries,
        }
    }

    fn canonical_elements(&self) -> Vec<DocumentElementId> {
        let mut elements = Vec::with_capacity(
            1 + self.points.len()
                + self.scalars.len()
                + self.curves.len()
                + self.contacts.len()
                + self.constraints.len() * 2
                + self.dimensions.len() * 2
                + self.parameters.len()
                + self.external_bindings.len(),
        );
        elements.push(DocumentElementId::Document(self.id));
        elements.extend(
            self.points
                .iter()
                .map(|value| DocumentElementId::Point(value.id)),
        );
        elements.extend(
            self.scalars
                .iter()
                .map(|value| DocumentElementId::Scalar(value.id)),
        );
        elements.extend(
            self.curves
                .iter()
                .map(|value| DocumentElementId::Curve(value.id)),
        );
        elements.extend(
            self.contacts
                .iter()
                .map(|value| DocumentElementId::Contact(value.id)),
        );
        elements.extend(
            self.constraints
                .iter()
                .map(|value| DocumentElementId::Constraint(value.id)),
        );
        elements.extend(
            self.dimensions
                .iter()
                .map(|value| DocumentElementId::Dimension(value.id)),
        );
        elements.extend(
            self.parameters
                .iter()
                .map(|value| DocumentElementId::Parameter(value.id)),
        );
        elements.extend(
            self.external_bindings
                .iter()
                .map(|value| DocumentElementId::ExternalBinding(value.id)),
        );
        elements.extend(
            self.source_order
                .iter()
                .copied()
                .map(DocumentElementId::Source),
        );
        elements.sort_by_key(|element| canonical_element_key(*element));
        elements
    }

    fn direct_inactivity_reason(&self, element: DocumentElementId) -> Option<InactivityReason> {
        let owner_suppressed = match element {
            DocumentElementId::Constraint(id) => self
                .constraint(id)
                .is_some_and(|constraint| constraint.suppressed),
            DocumentElementId::Dimension(id) => self
                .dimension(id)
                .is_some_and(|dimension| dimension.suppressed),
            DocumentElementId::Source(id) => {
                self.source(id).is_some_and(|source| source.suppressed)
            }
            _ => false,
        };
        if owner_suppressed || self.user_inactive_elements.contains(&element) {
            return Some(InactivityReason::UserSuppressed);
        }
        let activation = self.host_activation.as_ref()?;
        let owner_source = match element {
            DocumentElementId::Constraint(id) => self
                .constraint(id)
                .map(|constraint| DocumentElementId::Source(constraint.source_id)),
            DocumentElementId::Dimension(id) => self
                .dimension(id)
                .map(|dimension| DocumentElementId::Source(dimension.source_id)),
            _ => None,
        };
        activation.overrides().iter().find_map(|entry| {
            if entry.element() != element && Some(entry.element()) != owner_source {
                return None;
            }
            Some(match entry {
                HostActivationOverride::Inactive(_) => InactivityReason::HostConfigurationInactive,
                HostActivationOverride::UnavailableExternalReference(_) => {
                    InactivityReason::UnavailableExternalReference
                }
            })
        })
    }

    #[allow(clippy::too_many_lines)]
    fn direct_dependencies(&self, element: DocumentElementId) -> Vec<DocumentElementId> {
        if element == DocumentElementId::Document(self.id) {
            return Vec::new();
        }
        let mut dependencies = vec![DocumentElementId::Document(self.id)];
        let objects = self
            .points
            .iter()
            .map(|value| DocumentObjectId::Point(value.id))
            .chain(
                self.scalars
                    .iter()
                    .map(|value| DocumentObjectId::Scalar(value.id)),
            )
            .chain(
                self.curves
                    .iter()
                    .map(|value| DocumentObjectId::Curve(value.id)),
            )
            .chain(
                self.contacts
                    .iter()
                    .map(|value| DocumentObjectId::Contact(value.id)),
            )
            .chain(
                self.constraints
                    .iter()
                    .map(|value| DocumentObjectId::Constraint(value.id)),
            )
            .chain(
                self.dimensions
                    .iter()
                    .map(|value| DocumentObjectId::Dimension(value.id)),
            )
            .chain(
                self.parameters
                    .iter()
                    .map(|value| DocumentObjectId::Parameter(value.id)),
            )
            .chain(
                self.external_bindings
                    .iter()
                    .map(|value| DocumentObjectId::ExternalBinding(value.id)),
            );
        match element {
            DocumentElementId::Curve(id) => {
                if let Some(curve) = self.curve(id) {
                    dependencies.extend(
                        objects
                            .filter(|object| curve_references_object(&curve.definition, *object))
                            .map(document_object_element),
                    );
                }
            }
            DocumentElementId::Contact(id) => {
                if let Some(contact) = self.contact(id) {
                    dependencies.extend(
                        objects
                            .filter(|object| contact_references_object(contact, *object))
                            .map(document_object_element),
                    );
                }
            }
            DocumentElementId::Constraint(id) => {
                if let Some(constraint) = self.constraint(id) {
                    dependencies.extend(
                        objects
                            .filter(|object| {
                                constraint_references_object(&constraint.definition, *object)
                            })
                            .map(document_object_element),
                    );
                }
            }
            DocumentElementId::Dimension(id) => {
                if let Some(dimension) = self.dimension(id) {
                    dependencies.extend(
                        objects
                            .filter(|object| {
                                dimension_references_object(&dimension.definition, *object)
                            })
                            .map(document_object_element),
                    );
                }
            }
            DocumentElementId::Source(id) => {
                if let Some(source) = self.source(id) {
                    dependencies.push(match source.owner {
                        DocumentSourceOwner::Constraint(id) => id.into(),
                        DocumentSourceOwner::Dimension(id) => id.into(),
                    });
                }
            }
            DocumentElementId::Document(_)
            | DocumentElementId::Point(_)
            | DocumentElementId::Scalar(_)
            | DocumentElementId::Parameter(_)
            | DocumentElementId::ExternalBinding(_) => {}
        }
        dependencies.sort_by_key(|dependency| canonical_element_key(*dependency));
        dependencies.dedup();
        dependencies
    }

    fn line_span_endpoint_ids(
        &self,
        span: CurveSpan,
    ) -> Result<(DesignPointId, DesignPointId), DocumentError> {
        let curve = self
            .curve(span.curve)
            .ok_or_else(|| unknown("curve", span.curve.0))?;
        let endpoints = match &curve.definition {
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
        Ok(endpoints)
    }

    pub(crate) fn current_curve_span_direction(
        &self,
        span: CurveSpan,
    ) -> Result<[f64; 2], DocumentError> {
        let (start, end) = self.line_span_endpoint_ids(span)?;
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
            DocumentObjectId::Parameter(id) => self.parameter(id).is_some(),
            DocumentObjectId::ExternalBinding(id) => self.external_binding(id).is_some(),
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
        activity: &EffectiveActivity,
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
                if self.curve_branch_is_enforced_with_activity(CurveSpan::line(curve), activity)
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
                    if self.curve_branch_is_enforced_with_activity(
                        CurveSpan {
                            curve,
                            segment: u32::try_from(index).map_err(|_| {
                                DocumentError::ResourceLimit {
                                    resource: "polyline segment index",
                                    actual: index,
                                    limit: u32::MAX as usize,
                                }
                            })?,
                        },
                        activity,
                    ) && dot(
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
            C::CoincidentWithOrigin { point } | C::PointOnDatumAxis { point, .. } => {
                self.require_point(*point)?;
            }
            C::Coincident { first, second } => self.require_distinct_points(*first, *second)?,
            C::ExternalPointCoincident { point, external } => {
                self.require_point(*point)?;
                let binding = self
                    .external_binding(external.binding)
                    .ok_or_else(|| unknown("external binding", external.binding.0))?;
                if binding.expected_kind != ExternalFeatureKindV1::Point {
                    return invalid(
                        "external point operand",
                        "binding must expect a point feature",
                    );
                }
            }
            C::Horizontal { line } | C::Vertical { line } => {
                self.validate_line_span(*line)?;
            }
            C::HorizontalPoints { first, second }
            | C::VerticalPoints { first, second }
            | C::SymmetricAboutDatumAxis { first, second, .. } => {
                self.require_distinct_points(*first, *second)?;
            }
            C::HorizontalPointToMidpoint { point, line }
            | C::VerticalPointToMidpoint { point, line }
            | C::Midpoint { point, line } => {
                self.require_point(*point)?;
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
            C::ExternalLineCollinear { line, external } => {
                self.validate_line_span(line.span)?;
                let binding = self
                    .external_binding(external.binding)
                    .ok_or_else(|| unknown("external binding", external.binding.0))?;
                if binding.expected_kind != ExternalFeatureKindV1::LineSegment {
                    return invalid(
                        "external line operand",
                        "binding must expect a line-segment feature",
                    );
                }
            }
            C::CollinearWithDatumAxis { line, .. } => {
                self.validate_line_span(line.span)?;
            }
            C::Concentric { first, second } => {
                self.validate_center_ref(*first)?;
                self.validate_center_ref(*second)?;
                let first_center = self.resolve_center_ref(*first)?;
                let second_center = self.resolve_center_ref(*second)?;
                if first_center == second_center {
                    return invalid(
                        "concentric operands",
                        "curve centers must resolve to distinct stored points",
                    );
                }
            }
            C::Collinear { first, second } => {
                self.validate_line_support_ref(*first)?;
                self.validate_line_support_ref(*second)?;
                if first.span == second.span {
                    return invalid("collinear operands", "line supports must be distinct");
                }
            }
            C::EqualRadius { first, second } => {
                self.require_circle(*first)?;
                self.require_circle(*second)?;
                if first == second {
                    return invalid("constraint.definition", "circles must be distinct");
                }
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
            C::LineLineFillet {
                arc,
                first_contact,
                second_contact,
                ..
            } => {
                if first_contact == second_contact {
                    return invalid("fillet contact", "parent contacts must be distinct");
                }
                let first = self.require_line_contact(*first_contact)?;
                let second = self.require_line_contact(*second_contact)?;
                if first.curve == second.curve {
                    return invalid("fillet parent", "line spans must be distinct");
                }
                for contact in [first, second] {
                    if contact.domain
                        != (ContactDomain::Bounded {
                            lower: 0.0,
                            upper: 1.0,
                        })
                        || contact.neighborhood != ContactNeighborhood::Interior
                        || contact.tangent_orientation.is_some()
                    {
                        return invalid(
                            "fillet contact",
                            "parents require unoriented strict-interior bounded [0, 1] contacts",
                        );
                    }
                }
                let output = self.require_radial_curve(*arc)?;
                if !matches!(output.definition, CurveDefinition::CircularArc { .. }) {
                    return invalid("line fillet arc", "output must be a circular arc");
                }
                let first_jet = self.evaluate_contact_jet(*first_contact).map_err(|error| {
                    contact_document_evaluation_error(*first_contact, first.curve.curve, error)
                })?;
                let second_jet = self
                    .evaluate_contact_jet(*second_contact)
                    .map_err(|error| {
                        contact_document_evaluation_error(
                            *second_contact,
                            second.curve.curve,
                            error,
                        )
                    })?;
                let first_differential = first_jet.differential().map_err(|source| {
                    DocumentError::ContactDifferential {
                        contact: *first_contact,
                        source,
                    }
                })?;
                let second_differential = second_jet.differential().map_err(|source| {
                    DocumentError::ContactDifferential {
                        contact: *second_contact,
                        source,
                    }
                })?;
                if cross(
                    [
                        first_differential.unit_tangent.x,
                        first_differential.unit_tangent.y,
                    ],
                    [
                        second_differential.unit_tangent.x,
                        second_differential.unit_tangent.y,
                    ],
                )
                .abs()
                    <= 1.0e-8
                {
                    return invalid(
                        "fillet parent",
                        "line directions are parallel or numerically unresolved",
                    );
                }
            }
            C::CurveCurveFillet {
                arc,
                first_contact,
                first_side,
                second_contact,
                second_side,
                ..
            } => {
                if first_contact == second_contact {
                    return invalid("fillet contact", "parent contacts must be distinct");
                }
                let first = self.require_contact(*first_contact)?;
                let second = self.require_contact(*second_contact)?;
                if first.curve == second.curve {
                    return invalid("fillet parent", "support spans must be distinct");
                }
                for contact in [first, second] {
                    if contact.tangent_orientation.is_some()
                        || matches!(
                            contact.neighborhood,
                            ContactNeighborhood::Start | ContactNeighborhood::End
                        )
                    {
                        return invalid(
                            "fillet contact",
                            "parents require unoriented interior or finite-local roots",
                        );
                    }
                }
                let output = self.require_radial_curve(*arc)?;
                if !matches!(output.definition, CurveDefinition::CircularArc { .. }) {
                    return invalid("curve fillet arc", "output must be a circular arc");
                }
                if first.curve.curve == *arc || second.curve.curve == *arc {
                    return invalid("fillet parent", "output arc cannot be a parent support");
                }
                let first_differential = self.contact_differential(first)?;
                let second_differential = self.contact_differential(second)?;
                let tangent_cross = cross(
                    [
                        first_differential.unit_tangent.x,
                        first_differential.unit_tangent.y,
                    ],
                    [
                        second_differential.unit_tangent.x,
                        second_differential.unit_tangent.y,
                    ],
                );
                if !tangent_cross.is_finite() || tangent_cross.abs() <= 1.0e-8 {
                    return invalid(
                        "fillet parent",
                        "parent tangents are parallel or numerically unresolved",
                    );
                }
                let radius = self.curve_radius(output)?;
                for (side, curvature) in [
                    (*first_side, first_differential.signed_curvature),
                    (*second_side, second_differential.signed_curvature),
                ] {
                    let factor = 1.0 - fillet_side_sign(side) * radius * curvature;
                    if !factor.is_finite() || factor.abs() <= 1.0e-8 {
                        return invalid(
                            "fillet parent",
                            "parent offset factor is numerically unresolved",
                        );
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_dimension_definition(
        &self,
        definition: &DocumentDimensionDefinition,
        mode: DocumentDimensionMode,
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
            D::SupportingLineOffset {
                source,
                target_segment,
                target,
                ..
            }
            | D::ExactTranslatedSegmentOffset {
                source,
                target_segment,
                target,
                ..
            } => {
                self.validate_line_span(*source)?;
                self.validate_line_span(*target_segment)?;
                if source == target_segment {
                    return invalid("dimension.definition", "offset spans must be distinct");
                }
                *target
            }
            D::ProfileOffset { target, operand } => {
                if mode != DocumentDimensionMode::Driving {
                    return invalid(
                        "profile offset mode",
                        "a grouped profile offset must remain driving",
                    );
                }
                self.validate_profile_offset_operand(operand)?;
                *target
            }
        };
        let scalar = self.require_scalar(target)?;
        if mode == DocumentDimensionMode::Driving {
            finite_positive(scalar.value, "dimension target")?;
        } else {
            finite(scalar.value, "dimension target")?;
        }
        let unit = match definition {
            D::OrientedAngle { .. } => ScalarUnit::Angle,
            _ => ScalarUnit::Length,
        };
        let domain_is_valid = match mode {
            DocumentDimensionMode::Driving => scalar.domain == ScalarDomain::Positive,
            // Existing documents commonly retain the positive domain after changing a
            // dimension to reference mode. M42 also permits finite-only reference
            // storage because the scalar is not consumed as a solver coefficient.
            DocumentDimensionMode::Reference => {
                matches!(scalar.domain, ScalarDomain::Finite | ScalarDomain::Positive)
            }
        };
        if scalar.unit == unit && domain_is_valid {
            Ok(())
        } else {
            invalid(
                "dimension target",
                "scalar unit or domain does not match its semantic role",
            )
        }
    }

    fn validate_profile_offset_operand(
        &self,
        operand: &DocumentProfileOffsetOperand,
    ) -> Result<(), DocumentError> {
        let mut used = BTreeSet::new();
        match operand {
            DocumentProfileOffsetOperand::Face { outer, holes, .. } => {
                self.validate_profile_offset_path(&outer.edges, &outer.junctions, true, &mut used)?;
                for hole in holes {
                    self.validate_profile_offset_path(
                        &hole.edges,
                        &hole.junctions,
                        true,
                        &mut used,
                    )?;
                }
            }
            DocumentProfileOffsetOperand::OpenChain { chain, .. } => {
                self.validate_profile_offset_path(
                    &chain.edges,
                    &chain.junctions,
                    false,
                    &mut used,
                )?;
            }
        }
        Ok(())
    }

    fn validate_profile_offset_path(
        &self,
        edges: &[DocumentProfileOffsetEdgePair],
        junctions: &[DocumentProfileOffsetJunction],
        closed: bool,
        used: &mut BTreeSet<CurveSpan>,
    ) -> Result<(), DocumentError> {
        if edges.is_empty() {
            return invalid(
                "profile offset topology",
                "an operand path must contain at least one edge",
            );
        }
        let first_family = self.profile_offset_curve_family(edges[0].source.curve)?;
        let periodic_circle = edges.len() == 1 && first_family == ProfileOffsetCurveFamily::Circle;
        if periodic_circle && !closed {
            return invalid(
                "profile offset topology",
                "a full circle is available only as a closed face operand",
            );
        }
        let expected_junctions = if periodic_circle {
            0
        } else if closed {
            edges.len()
        } else {
            edges.len() - 1
        };
        if junctions.len() != expected_junctions {
            return invalid(
                "profile offset topology",
                "junction count does not match the ordered path",
            );
        }
        if !periodic_circle && edges.len() == 1 && closed {
            return invalid(
                "profile offset topology",
                "a one-edge closed loop must be a full circle",
            );
        }
        for edge in edges {
            let source_family = self.profile_offset_curve_family(edge.source.curve)?;
            let target_family = self.profile_offset_curve_family(edge.target.curve)?;
            if self.geometry_role(edge.source.curve.curve) != Some(GeometryRole::Profile)
                || self.geometry_role(edge.target.curve.curve) != Some(GeometryRole::Profile)
            {
                return invalid(
                    "profile offset geometry role",
                    "source and target supports must remain Profile geometry",
                );
            }
            if source_family != target_family {
                return invalid(
                    "profile offset edge pair",
                    "source and target supports must use the same exact curve family",
                );
            }
            if edge.source.curve == edge.target.curve
                || !used.insert(edge.source.curve)
                || !used.insert(edge.target.curve)
            {
                return invalid(
                    "profile offset edge pair",
                    "every source and target support must be distinct and occur exactly once",
                );
            }
            if source_family == ProfileOffsetCurveFamily::Circle && !periodic_circle {
                return invalid(
                    "profile offset topology",
                    "a full circle must be the only edge in its periodic path",
                );
            }
        }
        for (index, junction) in junctions.iter().enumerate() {
            let incoming = edges[index];
            let outgoing = edges[(index + 1) % edges.len()];
            self.validate_profile_offset_junction_owner(
                junction.source_owner,
                incoming.source,
                outgoing.source,
            )?;
            self.validate_profile_offset_junction_owner(
                junction.target_owner,
                incoming.target,
                outgoing.target,
            )?;
        }
        Ok(())
    }

    fn profile_offset_curve_family(
        &self,
        span: CurveSpan,
    ) -> Result<ProfileOffsetCurveFamily, DocumentError> {
        let curve = self.validate_span(span)?;
        Ok(match curve.definition {
            CurveDefinition::Line { .. } | CurveDefinition::Polyline { .. } => {
                ProfileOffsetCurveFamily::Line
            }
            CurveDefinition::CircularArc { .. } => ProfileOffsetCurveFamily::CircularArc,
            CurveDefinition::Circle { .. } => ProfileOffsetCurveFamily::Circle,
            _ => {
                return invalid(
                    "profile offset support",
                    "only lines, circular arcs, and full circles are supported",
                );
            }
        })
    }

    fn validate_profile_offset_junction_owner(
        &self,
        owner: DocumentProfileOffsetJunctionOwner,
        incoming: DocumentDirectedProfileOffsetCurve,
        outgoing: DocumentDirectedProfileOffsetCurve,
    ) -> Result<(), DocumentError> {
        match owner {
            DocumentProfileOffsetJunctionOwner::SharedPoint(point) => {
                self.require_point(point)?;
                let incoming_end = self.profile_offset_line_endpoint(incoming, false)?;
                let outgoing_start = self.profile_offset_line_endpoint(outgoing, true)?;
                if incoming_end != point || outgoing_start != point {
                    return invalid(
                        "profile offset junction owner",
                        "the retained shared point must own both directed line endpoints",
                    );
                }
            }
            DocumentProfileOffsetJunctionOwner::Constraint(owner) => {
                let constraint = self
                    .constraint(owner)
                    .ok_or_else(|| unknown("profile offset junction constraint", owner.0))?;
                let expected = [
                    (
                        incoming,
                        false,
                        profile_offset_endpoint_parameter(incoming.traversal, false),
                    ),
                    (
                        outgoing,
                        true,
                        profile_offset_endpoint_parameter(outgoing.traversal, true),
                    ),
                ];
                let both_contact_owned = expected.iter().all(|(directed, _, parameter)| {
                    constraint_contacts(&constraint.definition)
                        .into_iter()
                        .any(|contact| {
                            self.profile_offset_contact_matches(contact, *directed, *parameter)
                        })
                });
                let owns_pair = match constraint.definition {
                    DocumentConstraintDefinition::Coincident { first, second } => self
                        .profile_offset_line_endpoint(incoming, false)
                        .ok()
                        .zip(self.profile_offset_line_endpoint(outgoing, true).ok())
                        .is_some_and(|(incoming, outgoing)| {
                            (incoming == first && outgoing == second)
                                || (incoming == second && outgoing == first)
                        }),
                    DocumentConstraintDefinition::PointOnCurve { point, contact } => {
                        [(0, 1), (1, 0)]
                            .into_iter()
                            .any(|(curve_index, point_index)| {
                                let (curve, _, parameter) = expected[curve_index];
                                let (point_curve, directed_start, _) = expected[point_index];
                                self.profile_offset_contact_matches(contact, curve, parameter)
                                    && self
                                        .profile_offset_line_endpoint(point_curve, directed_start)
                                        .is_ok_and(|endpoint| endpoint == point)
                            })
                    }
                    DocumentConstraintDefinition::LineCurveTangency {
                        line,
                        endpoint,
                        curve_contact,
                    } => [(0, 1), (1, 0)]
                        .into_iter()
                        .any(|(line_index, curve_index)| {
                            let (expected_line, _, line_parameter) = expected[line_index];
                            let (expected_curve, _, curve_parameter) = expected[curve_index];
                            let native_line_parameter = match endpoint {
                                FeatureEndpoint::Start => 0.0,
                                FeatureEndpoint::End => 1.0,
                            };
                            expected_line.curve == line
                                && (line_parameter - native_line_parameter).abs()
                                    <= 64.0 * f64::EPSILON
                                && self.profile_offset_contact_matches(
                                    curve_contact,
                                    expected_curve,
                                    curve_parameter,
                                )
                        }),
                    DocumentConstraintDefinition::LineCircleTangency { .. }
                    | DocumentConstraintDefinition::CircleArcTangency { .. }
                    | DocumentConstraintDefinition::CurveCurveContact { .. }
                    | DocumentConstraintDefinition::CurveCurveTangency { .. }
                    | DocumentConstraintDefinition::EndpointContinuity { .. } => both_contact_owned,
                    _ => false,
                };
                if !owns_pair {
                    return invalid(
                        "profile offset junction owner",
                        "the retained constraint must own both exact directed endpoints",
                    );
                }
            }
        }
        Ok(())
    }

    fn profile_offset_contact_matches(
        &self,
        contact: ContactId,
        directed: DocumentDirectedProfileOffsetCurve,
        parameter: f64,
    ) -> bool {
        self.contact(contact).is_some_and(|contact| {
            let neighborhood_matches = if parameter.to_bits() == 0.0_f64.to_bits() {
                matches!(contact.neighborhood, ContactNeighborhood::Start)
            } else if parameter.to_bits() == 1.0_f64.to_bits() {
                matches!(contact.neighborhood, ContactNeighborhood::End)
            } else {
                false
            };
            contact.curve == directed.curve
                && contact.domain
                    == (ContactDomain::Bounded {
                        lower: 0.0,
                        upper: 1.0,
                    })
                && contact.winding == 0
                && neighborhood_matches
                && self
                    .scalar(contact.parameter)
                    .is_some_and(|scalar| scalar.value.to_bits() == parameter.to_bits())
        })
    }

    fn profile_offset_line_endpoint(
        &self,
        curve: DocumentDirectedProfileOffsetCurve,
        start: bool,
    ) -> Result<DesignPointId, DocumentError> {
        let (native_start, native_end) = self.line_span_endpoint_ids(curve.curve)?;
        Ok(match (curve.traversal, start) {
            (DocumentOffsetTraversal::Forward, true)
            | (DocumentOffsetTraversal::Reverse, false) => native_start,
            (DocumentOffsetTraversal::Forward, false)
            | (DocumentOffsetTraversal::Reverse, true) => native_end,
        })
    }

    fn validate_fillet_parent_request(
        &self,
        parent: CurveFilletParentRequest,
    ) -> Result<geosolve_geometry::CurveJet2, DocumentError> {
        self.validate_span(parent.curve)?;
        finite(parent.parameter, "curve fillet contact parameter")?;
        let periodic = self.trim_support_is_periodic(parent.curve)?;
        let period = self.trim_support_period(parent.curve)?;
        let total = if periodic {
            if !(0.0..period).contains(&parent.parameter) {
                return invalid(
                    "curve fillet contact parameter",
                    "periodic principal value must be in [0, period)",
                );
            }
            let anchor = parent
                .periodic_anchor
                .ok_or_else(|| DocumentError::InvalidField {
                    field: "curve fillet periodic anchor",
                    message: "a full circle or ellipse requires an explicit anchor".into(),
                })?;
            let anchor_total = self.resolve_fixed_trim_parameter(parent.curve, anchor)?;
            let contact_total = parent.parameter + f64::from(parent.winding) * period;
            let (start, end) = match parent.trim_endpoint {
                DocumentFilletTrimEndpoint::Start => (contact_total, anchor_total),
                DocumentFilletTrimEndpoint::End => (anchor_total, contact_total),
            };
            if !start.is_finite() || !end.is_finite() || start >= end || end - start > period {
                return invalid(
                    "curve fillet periodic anchor",
                    "anchor and contact must define an increasing interval no wider than one period",
                );
            }
            contact_total
        } else {
            let allows_winding = self.trim_support_allows_winding(parent.curve)?;
            if (!allows_winding && parent.winding != 0)
                || parent.periodic_anchor.is_some()
                || !(0.0 < parent.parameter && parent.parameter < 1.0)
            {
                return invalid(
                    "curve fillet contact parameter",
                    "bounded support requires a strict-interior parameter, compatible winding, and no periodic anchor",
                );
            }
            parent.parameter
        };
        finite(total, "curve fillet total parameter")?;
        match parent.neighborhood {
            ContactNeighborhood::Interior => {}
            ContactNeighborhood::Local { lower, upper }
                if lower.is_finite() && upper.is_finite() && lower < total && total < upper => {}
            ContactNeighborhood::Local { .. } => {
                return invalid(
                    "curve fillet contact neighborhood",
                    "finite local bounds must strictly contain the unwrapped root",
                );
            }
            ContactNeighborhood::Start | ContactNeighborhood::End => {
                return invalid(
                    "curve fillet contact neighborhood",
                    "fillet parents cannot select support endpoints",
                );
            }
        }
        let jet = self
            .evaluate_curve_jet(parent.curve, total)
            .map_err(|error| DocumentError::InvalidField {
                field: "curve fillet parent",
                message: error.to_string(),
            })?;
        jet.differential()
            .map_err(|error| DocumentError::InvalidField {
                field: "curve fillet parent",
                message: error.to_string(),
            })?;
        Ok(jet)
    }

    fn fillet_trim_view(
        &self,
        parent: CurveFilletParentRequest,
        owner: DocumentConstraintId,
        contact: ContactId,
    ) -> Result<DocumentCurveTrimView, DocumentError> {
        let periodic = self.trim_support_is_periodic(parent.curve)?;
        let fixed = if periodic {
            parent
                .periodic_anchor
                .ok_or_else(|| DocumentError::InvalidField {
                    field: "curve fillet periodic anchor",
                    message: "a full circle or ellipse requires an explicit anchor".into(),
                })?
        } else {
            DocumentTrimParameter {
                parameter: match parent.trim_endpoint {
                    DocumentFilletTrimEndpoint::Start => 1.0,
                    DocumentFilletTrimEndpoint::End => 0.0,
                },
                winding: if self.trim_support_allows_winding(parent.curve)? {
                    parent.winding
                } else {
                    0
                },
            }
        };
        let contact_boundary = DocumentTrimBoundary::FilletContact { owner, contact };
        let fixed = DocumentTrimBoundary::Fixed(fixed);
        let (start, end) = match parent.trim_endpoint {
            DocumentFilletTrimEndpoint::Start => (contact_boundary, fixed),
            DocumentFilletTrimEndpoint::End => (fixed, contact_boundary),
        };
        Ok(DocumentCurveTrimView {
            support: parent.curve,
            start,
            end,
        })
    }

    fn trim_support_is_periodic(&self, support: CurveSpan) -> Result<bool, DocumentError> {
        Ok(matches!(
            self.validate_span(support)?.definition,
            CurveDefinition::Circle { .. } | CurveDefinition::Ellipse { .. }
        ))
    }

    fn trim_support_allows_winding(&self, support: CurveSpan) -> Result<bool, DocumentError> {
        Ok(matches!(
            self.validate_span(support)?.definition,
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
        ))
    }

    fn trim_support_period(&self, support: CurveSpan) -> Result<f64, DocumentError> {
        Ok(if self.trim_support_is_periodic(support)? {
            std::f64::consts::TAU
        } else {
            1.0
        })
    }

    fn resolve_fixed_trim_parameter(
        &self,
        support: CurveSpan,
        parameter: DocumentTrimParameter,
    ) -> Result<f64, DocumentError> {
        finite(parameter.parameter, "trim boundary parameter")?;
        let periodic = self.trim_support_is_periodic(support)?;
        let period = self.trim_support_period(support)?;
        let total = if periodic {
            if !(0.0..period).contains(&parameter.parameter) {
                return invalid(
                    "trim boundary parameter",
                    "periodic principal value must be in [0, period)",
                );
            }
            parameter.parameter + f64::from(parameter.winding) * period
        } else {
            if (!self.trim_support_allows_winding(support)? && parameter.winding != 0)
                || !(0.0..=1.0).contains(&parameter.parameter)
            {
                return invalid(
                    "trim boundary parameter",
                    "bounded support requires a parameter in [0, 1] with compatible winding",
                );
            }
            parameter.parameter
        };
        finite(total, "unwrapped trim boundary")?;
        Ok(total)
    }

    fn resolve_trim_boundary(
        &self,
        support: CurveSpan,
        boundary: DocumentTrimBoundary,
    ) -> Result<f64, DocumentError> {
        match boundary {
            DocumentTrimBoundary::Fixed(parameter) => {
                self.resolve_fixed_trim_parameter(support, parameter)
            }
            DocumentTrimBoundary::FilletContact { owner, contact } => {
                let constraint = self
                    .constraint(owner)
                    .ok_or_else(|| unknown("trim boundary owner", owner.0))?;
                let owns_contact = matches!(
                    constraint.definition,
                    DocumentConstraintDefinition::CurveCurveFillet {
                        first_contact,
                        second_contact,
                        ..
                    } if first_contact == contact || second_contact == contact
                );
                if !owns_contact {
                    return invalid(
                        "trim boundary owner",
                        "owner is not the generic fillet that owns this contact",
                    );
                }
                let slot = self.require_contact(contact)?;
                if slot.curve != support {
                    return invalid(
                        "trim boundary contact",
                        "contact support does not match the trim view",
                    );
                }
                let principal = self.require_scalar(slot.parameter)?.value;
                let parameter = DocumentTrimParameter {
                    parameter: principal,
                    winding: slot.winding,
                };
                self.resolve_fixed_trim_parameter(support, parameter)
            }
            DocumentTrimBoundary::ConstraintContact { owner, contact } => {
                let constraint = self
                    .constraint(owner)
                    .ok_or_else(|| unknown("trim boundary owner", owner.0))?;
                if !matches!(
                    constraint.definition,
                    DocumentConstraintDefinition::PointOnCurve {
                        contact: owned,
                        ..
                    } if owned == contact
                ) {
                    return invalid(
                        "trim boundary owner",
                        "owner is not a point-on-curve constraint for this contact",
                    );
                }
                let slot = self.require_contact(contact)?;
                if slot.curve != support {
                    return invalid(
                        "trim boundary contact",
                        "contact support does not match the trim view",
                    );
                }
                self.resolve_fixed_trim_parameter(
                    support,
                    DocumentTrimParameter {
                        parameter: self.require_scalar(slot.parameter)?.value,
                        winding: slot.winding,
                    },
                )
            }
        }
    }

    fn resolve_trim_view(
        &self,
        view: &DocumentCurveTrimView,
    ) -> Result<DocumentVisibleCurveInterval, DocumentError> {
        self.validate_span(view.support)?;
        let start = self.resolve_trim_boundary(view.support, view.start)?;
        let end = self.resolve_trim_boundary(view.support, view.end)?;
        let period = self.trim_support_period(view.support)?;
        if !start.is_finite() || !end.is_finite() || start >= end || end - start > period {
            return invalid(
                "trim view interval",
                "must be finite, increasing, and no wider than one native period",
            );
        }
        Ok(DocumentVisibleCurveInterval {
            support: view.support,
            start,
            end,
            start_boundary: view.start,
            end_boundary: view.end,
        })
    }

    fn validate_owned_trim_boundary(
        &self,
        owner: DocumentConstraintId,
        contact: ContactId,
        endpoint: DocumentFilletTrimEndpoint,
    ) -> Result<(), DocumentError> {
        let slot = self.require_contact(contact)?;
        let expected = DocumentTrimBoundary::FilletContact { owner, contact };
        let view = self
            .trim_views_for_span(slot.curve)
            .find(|view| view.start == expected || view.end == expected)
            .ok_or_else(|| DocumentError::InvalidField {
                field: "trim view ownership",
                message: "generic fillet parent has no trim view".into(),
            })?;
        let (contact_boundary, opposite) = match endpoint {
            DocumentFilletTrimEndpoint::Start => (view.start, view.end),
            DocumentFilletTrimEndpoint::End => (view.end, view.start),
        };
        let DocumentTrimBoundary::Fixed(fixed) = opposite else {
            return invalid(
                "trim view ownership",
                "generic fillet must own exactly its requested endpoint and one fixed opposite boundary",
            );
        };
        if contact_boundary != expected {
            return invalid(
                "trim view ownership",
                "generic fillet must own exactly its requested endpoint and one fixed opposite boundary",
            );
        }
        if !self.trim_support_is_periodic(slot.curve)? {
            let expected_fixed: f64 = match endpoint {
                DocumentFilletTrimEndpoint::Start => 1.0,
                DocumentFilletTrimEndpoint::End => 0.0,
            };
            let expected_winding = if self.trim_support_allows_winding(slot.curve)? {
                slot.winding
            } else {
                0
            };
            if fixed.winding != expected_winding
                || fixed.parameter.to_bits() != expected_fixed.to_bits()
            {
                return invalid(
                    "trim view ownership",
                    "bounded generic fillet support must retain the opposite native endpoint",
                );
            }
        }
        Ok(())
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

fn push_curve_control(
    controls: &mut Vec<DocumentCurveControl>,
    curve: CurveId,
    kind: DocumentCurveControlKind,
    position: [f64; 2],
    target: DocumentCurveControlTarget,
    availability: DocumentCurveControlAvailability,
) -> Result<(), DocumentCurveControlError> {
    let id = DocumentCurveControlId { curve, kind };
    if !position.iter().all(|value| value.is_finite()) {
        return Err(DocumentCurveControlError::NonFiniteResult { control: id });
    }
    controls.push(DocumentCurveControl {
        id,
        position,
        target,
        availability,
    });
    Ok(())
}

fn push_trim_controls(
    document: &SketchDocument,
    controls: &mut Vec<DocumentCurveControl>,
    curve: CurveId,
    start_position: [f64; 2],
    end_position: [f64; 2],
    definition: &CurveDefinition,
    availability: DocumentCurveControlAvailability,
) -> Result<(), DocumentCurveControlError> {
    let (start, end) = match definition {
        CurveDefinition::CircularArc {
            start_angle,
            end_angle,
            ..
        }
        | CurveDefinition::EllipticalArc {
            start_angle,
            end_angle,
            ..
        } => (*start_angle, *end_angle),
        CurveDefinition::ParabolaSegment {
            trim_start,
            trim_end,
            ..
        }
        | CurveDefinition::HyperbolaSegment {
            trim_start,
            trim_end,
            ..
        } => (*trim_start, *trim_end),
        _ => {
            return Err(DocumentCurveControlError::UnknownControl {
                curve,
                kind: DocumentCurveControlKind::TrimStart,
            });
        }
    };
    push_curve_control(
        controls,
        curve,
        DocumentCurveControlKind::TrimStart,
        start_position,
        DocumentCurveControlTarget::Scalar(start),
        document.scalar_control_availability(start, availability),
    )?;
    push_curve_control(
        controls,
        curve,
        DocumentCurveControlKind::TrimEnd,
        end_position,
        DocumentCurveControlTarget::Scalar(end),
        document.scalar_control_availability(end, availability),
    )
}

fn push_axis_controls(
    document: &SketchDocument,
    controls: &mut Vec<DocumentCurveControl>,
    curve: CurveId,
    center: DesignPointId,
    major_axis_point: DesignPointId,
    minor_axis_ratio: DesignScalarId,
    availability: DocumentCurveControlAvailability,
) -> Result<(), DocumentCurveControlError> {
    let definition = &document
        .curve(curve)
        .ok_or(DocumentCurveControlError::UnknownControl {
            curve,
            kind: DocumentCurveControlKind::MinorAxis,
        })?
        .definition;
    // Elliptical-arc trims may occupy either signed minor pole. Put the size
    // grip on the pole whose nearest trim endpoint is farther away, keeping the
    // ordinary positive pole for full ellipses and deterministic arc ties.
    let minor_axis_endpoint = if matches!(definition, CurveDefinition::EllipticalArc { .. }) {
        let start = document.evaluate_conic_feature(
            curve,
            DocumentConicFeature::BoundedEndpoint {
                endpoint: FeatureEndpoint::Start,
            },
        )?;
        let end = document.evaluate_conic_feature(
            curve,
            DocumentConicFeature::BoundedEndpoint {
                endpoint: FeatureEndpoint::End,
            },
        )?;
        let separation = |endpoint| -> Result<f64, DocumentCurveControlError> {
            let position = document.evaluate_conic_feature(
                curve,
                DocumentConicFeature::MinorAxisEndpoint { endpoint },
            )?;
            let distance = |trim: [f64; 2]| (position[0] - trim[0]).hypot(position[1] - trim[1]);
            Ok(distance(start).min(distance(end)))
        };
        let negative = separation(FeatureEndpoint::Start)?;
        let positive = separation(FeatureEndpoint::End)?;
        if negative > positive {
            FeatureEndpoint::Start
        } else {
            FeatureEndpoint::End
        }
    } else {
        FeatureEndpoint::End
    };
    push_curve_control(
        controls,
        curve,
        DocumentCurveControlKind::Center,
        document.require_point(center)?.position,
        DocumentCurveControlTarget::Point(center),
        availability,
    )?;
    push_curve_control(
        controls,
        curve,
        DocumentCurveControlKind::MajorAxisPoint,
        document.require_point(major_axis_point)?.position,
        DocumentCurveControlTarget::Point(major_axis_point),
        availability,
    )?;
    push_curve_control(
        controls,
        curve,
        DocumentCurveControlKind::MinorAxis,
        document.evaluate_conic_feature(
            curve,
            DocumentConicFeature::MinorAxisEndpoint {
                endpoint: minor_axis_endpoint,
            },
        )?,
        DocumentCurveControlTarget::Scalar(minor_axis_ratio),
        document.scalar_control_availability(minor_axis_ratio, availability),
    )
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

pub(crate) enum DocumentConicGeometryError {
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
        (
            ContactDomain::SupportingLine | ContactDomain::Periodic { .. },
            ContactNeighborhood::Local { lower, upper },
        ) if lower.is_finite()
            && upper.is_finite()
            && lower < contact_total_value(contact, scalar.value)
            && contact_total_value(contact, scalar.value) < upper => {}
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

fn validate_legacy_contact_language(contacts: &[ContactSlot]) -> Result<(), DocumentError> {
    if contacts.iter().any(|contact| {
        matches!(
            contact.domain,
            ContactDomain::SupportingLine | ContactDomain::Periodic { .. }
        ) && matches!(contact.neighborhood, ContactNeighborhood::Local { .. })
    }) {
        return invalid(
            "contact.neighborhood",
            "sketch versions 1 through 3 do not support local periodic or supporting-line neighborhoods",
        );
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
        }
        | DocumentConstraintDefinition::LineLineFillet {
            first_contact,
            second_contact,
            ..
        }
        | DocumentConstraintDefinition::CurveCurveFillet {
            first_contact,
            second_contact,
            ..
        } => vec![*first_contact, *second_contact],
        _ => Vec::new(),
    }
}

const fn is_retained_planar_constraint(definition: &DocumentConstraintDefinition) -> bool {
    matches!(
        definition,
        DocumentConstraintDefinition::CoincidentWithOrigin { .. }
            | DocumentConstraintDefinition::PointOnDatumAxis { .. }
            | DocumentConstraintDefinition::HorizontalPoints { .. }
            | DocumentConstraintDefinition::VerticalPoints { .. }
            | DocumentConstraintDefinition::HorizontalPointToMidpoint { .. }
            | DocumentConstraintDefinition::VerticalPointToMidpoint { .. }
            | DocumentConstraintDefinition::Concentric { .. }
            | DocumentConstraintDefinition::Collinear { .. }
            | DocumentConstraintDefinition::CollinearWithDatumAxis { .. }
            | DocumentConstraintDefinition::SymmetricAboutDatumAxis { .. }
    )
}

const fn is_datum_constraint(definition: &DocumentConstraintDefinition) -> bool {
    matches!(
        definition,
        DocumentConstraintDefinition::CoincidentWithOrigin { .. }
            | DocumentConstraintDefinition::PointOnDatumAxis { .. }
            | DocumentConstraintDefinition::CollinearWithDatumAxis { .. }
            | DocumentConstraintDefinition::SymmetricAboutDatumAxis { .. }
    )
}

#[allow(clippy::too_many_lines)]
const fn canonical_element_key(element: DocumentElementId) -> (u128, u8) {
    let kind = match element {
        DocumentElementId::Document(_) => 0,
        DocumentElementId::Point(_) => 1,
        DocumentElementId::Scalar(_) => 2,
        DocumentElementId::Curve(_) => 3,
        DocumentElementId::Contact(_) => 4,
        DocumentElementId::Constraint(_) => 5,
        DocumentElementId::Dimension(_) => 6,
        DocumentElementId::Parameter(_) => 7,
        DocumentElementId::ExternalBinding(_) => 8,
        DocumentElementId::Source(_) => 9,
    };
    (element.persistent_id().as_u128(), kind)
}

/// Canonical identity of a typed parameter target. A validated dimensionless
/// declaration has exactly one permissible unit/domain/branch interpretation for
/// its persistent scalar, so its scalar identity is the complete deduplication key.
pub(crate) const fn canonical_parameter_target_key(
    target: DocumentParameterTarget,
) -> (u8, u128, u8) {
    match target {
        DocumentParameterTarget::DrivingDimension(id) => (0, id.0.as_u128(), 0),
        DocumentParameterTarget::DimensionlessFixedScalar(property) => {
            (1, property.scalar.0.as_u128(), 0)
        }
        DocumentParameterTarget::Activation(element) => {
            let (id, kind) = canonical_element_key(element);
            (2, id, kind)
        }
    }
}

const fn document_object_element(object: DocumentObjectId) -> DocumentElementId {
    match object {
        DocumentObjectId::Point(id) => DocumentElementId::Point(id),
        DocumentObjectId::Scalar(id) => DocumentElementId::Scalar(id),
        DocumentObjectId::Curve(id) => DocumentElementId::Curve(id),
        DocumentObjectId::Contact(id) => DocumentElementId::Contact(id),
        DocumentObjectId::Constraint(id) => DocumentElementId::Constraint(id),
        DocumentObjectId::Dimension(id) => DocumentElementId::Dimension(id),
        DocumentObjectId::Parameter(id) => DocumentElementId::Parameter(id),
        DocumentObjectId::ExternalBinding(id) => DocumentElementId::ExternalBinding(id),
    }
}

fn activation_digest(revision: u64, overrides: &[HostActivationOverride]) -> ActivationDigest {
    // Four independently seeded stable FNV-1a lanes. This is a canonical identity,
    // not an authentication primitive; hosts that need authenticity sign the payload.
    let mut lanes = [
        0xcbf2_9ce4_8422_2325_u64,
        0x8422_2325_cbf2_9ce4_u64,
        0x9e37_79b1_85eb_ca87_u64,
        0xd6e8_feb8_6659_fd93_u64,
    ];
    let mut bytes = Vec::with_capacity(8 + overrides.len() * 18);
    bytes.extend_from_slice(&revision.to_be_bytes());
    for entry in overrides {
        let (state, element) = match entry {
            HostActivationOverride::Inactive(element) => (0_u8, *element),
            HostActivationOverride::UnavailableExternalReference(element) => (1_u8, *element),
        };
        let (_, kind) = canonical_element_key(element);
        bytes.push(state);
        bytes.push(kind);
        bytes.extend_from_slice(&element.persistent_id().as_u128().to_be_bytes());
    }
    for (lane_index, lane) in lanes.iter_mut().enumerate() {
        for byte in &bytes {
            *lane ^= u64::from(*byte).wrapping_add((lane_index as u64) << 8);
            *lane = lane.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    let mut digest = [0_u8; 32];
    for (index, lane) in lanes.into_iter().enumerate() {
        digest[index * 8..index * 8 + 8].copy_from_slice(&lane.to_be_bytes());
    }
    ActivationDigest(digest)
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
            | DocumentConstraintDefinition::CoincidentWithOrigin { point }
            | DocumentConstraintDefinition::PointOnDatumAxis { point, .. }
            | DocumentConstraintDefinition::PointOnCurve { point, .. }
            | DocumentConstraintDefinition::Midpoint { point, .. }
            | DocumentConstraintDefinition::HorizontalPointToMidpoint { point, .. }
            | DocumentConstraintDefinition::VerticalPointToMidpoint { point, .. }
            | DocumentConstraintDefinition::ExternalPointCoincident { point, .. },
            DocumentObjectId::Point(selected),
        ) => *point == selected,
        (
            DocumentConstraintDefinition::Coincident { first, second }
            | DocumentConstraintDefinition::HorizontalPoints { first, second }
            | DocumentConstraintDefinition::VerticalPoints { first, second }
            | DocumentConstraintDefinition::SymmetricAboutDatumAxis { first, second, .. }
            | DocumentConstraintDefinition::SymmetricAboutLine { first, second, .. },
            DocumentObjectId::Point(selected),
        ) => *first == selected || *second == selected,
        (
            DocumentConstraintDefinition::ExternalPointCoincident { external, .. },
            DocumentObjectId::ExternalBinding(selected),
        ) => external.binding == selected,
        (
            DocumentConstraintDefinition::Horizontal { line }
            | DocumentConstraintDefinition::Vertical { line }
            | DocumentConstraintDefinition::Midpoint { line, .. }
            | DocumentConstraintDefinition::HorizontalPointToMidpoint { line, .. }
            | DocumentConstraintDefinition::VerticalPointToMidpoint { line, .. }
            | DocumentConstraintDefinition::SymmetricAboutLine { line, .. }
            | DocumentConstraintDefinition::LineCurveTangency { line, .. }
            | DocumentConstraintDefinition::CurveDirection { line, .. },
            DocumentObjectId::Curve(selected),
        ) => line.curve == selected,
        (
            DocumentConstraintDefinition::ExternalLineCollinear { line, .. }
            | DocumentConstraintDefinition::CollinearWithDatumAxis { line, .. },
            DocumentObjectId::Curve(selected),
        ) => line.span.curve == selected,
        (
            DocumentConstraintDefinition::Concentric { first, second },
            DocumentObjectId::Curve(selected),
        ) => first.curve == selected || second.curve == selected,
        (
            DocumentConstraintDefinition::Collinear { first, second },
            DocumentObjectId::Curve(selected),
        ) => first.span.curve == selected || second.span.curve == selected,
        (
            DocumentConstraintDefinition::ExternalLineCollinear { external, .. },
            DocumentObjectId::ExternalBinding(selected),
        ) => external.binding == selected,
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
            DocumentConstraintDefinition::LineLineFillet { arc, .. }
            | DocumentConstraintDefinition::CurveCurveFillet { arc, .. },
            DocumentObjectId::Curve(selected),
        ) => *arc == selected,
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
            }
            | DocumentConstraintDefinition::LineLineFillet {
                first_contact,
                second_contact,
                ..
            }
            | DocumentConstraintDefinition::CurveCurveFillet {
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
        (
            DocumentDimensionDefinition::SupportingLineOffset {
                source,
                target_segment,
                ..
            }
            | DocumentDimensionDefinition::ExactTranslatedSegmentOffset {
                source,
                target_segment,
                ..
            },
            DocumentObjectId::Curve(selected),
        ) => source.curve == selected || target_segment.curve == selected,
        (
            DocumentDimensionDefinition::ProfileOffset { operand, .. },
            DocumentObjectId::Curve(selected),
        ) => document_profile_offset_edges(operand)
            .any(|edge| edge.source.curve.curve == selected || edge.target.curve.curve == selected),
        (
            DocumentDimensionDefinition::ProfileOffset { operand, .. },
            DocumentObjectId::Constraint(selected),
        ) => document_profile_offset_junctions(operand).any(|junction| {
            junction.source_owner == DocumentProfileOffsetJunctionOwner::Constraint(selected)
                || junction.target_owner == DocumentProfileOffsetJunctionOwner::Constraint(selected)
        }),
        (
            DocumentDimensionDefinition::ProfileOffset { operand, .. },
            DocumentObjectId::Point(selected),
        ) => document_profile_offset_junctions(operand).any(|junction| {
            junction.source_owner == DocumentProfileOffsetJunctionOwner::SharedPoint(selected)
                || junction.target_owner
                    == DocumentProfileOffsetJunctionOwner::SharedPoint(selected)
        }),
        (definition, DocumentObjectId::Scalar(scalar)) => dimension_target(definition) == scalar,
        _ => false,
    }
}

fn document_profile_offset_edges(
    operand: &DocumentProfileOffsetOperand,
) -> impl Iterator<Item = &DocumentProfileOffsetEdgePair> {
    let (first, rest): (
        &[DocumentProfileOffsetEdgePair],
        Vec<&[DocumentProfileOffsetEdgePair]>,
    ) = match operand {
        DocumentProfileOffsetOperand::Face { outer, holes, .. } => (
            &outer.edges,
            holes.iter().map(|value| value.edges.as_slice()).collect(),
        ),
        DocumentProfileOffsetOperand::OpenChain { chain, .. } => (&chain.edges, Vec::new()),
    };
    first.iter().chain(rest.into_iter().flatten())
}

fn document_profile_offset_junctions(
    operand: &DocumentProfileOffsetOperand,
) -> impl Iterator<Item = &DocumentProfileOffsetJunction> {
    let (first, rest): (
        &[DocumentProfileOffsetJunction],
        Vec<&[DocumentProfileOffsetJunction]>,
    ) = match operand {
        DocumentProfileOffsetOperand::Face { outer, holes, .. } => (
            &outer.junctions,
            holes
                .iter()
                .map(|value| value.junctions.as_slice())
                .collect(),
        ),
        DocumentProfileOffsetOperand::OpenChain { chain, .. } => (&chain.junctions, Vec::new()),
    };
    first.iter().chain(rest.into_iter().flatten())
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

fn point_defined_curve_controls(definition: &CurveDefinition) -> Option<Vec<DesignPointId>> {
    match definition {
        CurveDefinition::Line { start, end, .. } => Some(vec![*start, *end]),
        CurveDefinition::Polyline { points, .. } => Some(points.clone()),
        CurveDefinition::QuadraticBezier { controls } => Some(controls.to_vec()),
        CurveDefinition::CubicBezier { controls } => Some(controls.to_vec()),
        CurveDefinition::BSpline { controls, .. } => Some(controls.clone()),
        _ => None,
    }
}

fn mirror_curve_definition(
    definition: CurveDefinition,
    controls: &[DesignPointId],
    axis_direction: [f64; 2],
) -> Result<CurveDefinition, DocumentError> {
    match definition {
        CurveDefinition::Line {
            branch_direction, ..
        } => {
            let [start, end] = controls else {
                return invalid("mirrored curve", "line requires two mirrored points");
            };
            Ok(CurveDefinition::Line {
                start: *start,
                end: *end,
                branch_direction: reflect_direction_about_axis(branch_direction, axis_direction)?,
            })
        }
        CurveDefinition::Polyline {
            closed,
            branch_directions,
            ..
        } => Ok(CurveDefinition::Polyline {
            points: controls.to_vec(),
            closed,
            branch_directions: branch_directions
                .into_iter()
                .map(|direction| reflect_direction_about_axis(direction, axis_direction))
                .collect::<Result<Vec<_>, _>>()?,
        }),
        CurveDefinition::QuadraticBezier { .. } => Ok(CurveDefinition::QuadraticBezier {
            controls: controls
                .try_into()
                .map_err(|_| DocumentError::InvalidField {
                    field: "mirrored curve",
                    message: "quadratic Bezier requires three mirrored points".into(),
                })?,
        }),
        CurveDefinition::CubicBezier { .. } => Ok(CurveDefinition::CubicBezier {
            controls: controls
                .try_into()
                .map_err(|_| DocumentError::InvalidField {
                    field: "mirrored curve",
                    message: "cubic Bezier requires four mirrored points".into(),
                })?,
        }),
        CurveDefinition::BSpline {
            form,
            degree,
            knots,
            span_ids,
            next_span_id,
            ..
        } => Ok(CurveDefinition::BSpline {
            form,
            degree,
            controls: controls.to_vec(),
            knots,
            span_ids,
            next_span_id,
        }),
        _ => invalid(
            "mirror source",
            "expected a line, polyline, Bezier, or non-rational B-spline",
        ),
    }
}

fn reflect_point_about_line(
    point: [f64; 2],
    axis_origin: [f64; 2],
    axis_direction: [f64; 2],
) -> Result<[f64; 2], DocumentError> {
    let offset = [point[0] - axis_origin[0], point[1] - axis_origin[1]];
    let projection = offset[0] * axis_direction[0] + offset[1] * axis_direction[1];
    let reflected = [
        axis_origin[0] + 2.0 * projection * axis_direction[0] - offset[0],
        axis_origin[1] + 2.0 * projection * axis_direction[1] - offset[1],
    ];
    finite_pair(reflected, "mirrored point")?;
    Ok(reflected)
}

fn reflect_direction_about_axis(
    direction: [f64; 2],
    axis_direction: [f64; 2],
) -> Result<[f64; 2], DocumentError> {
    let projection = direction[0] * axis_direction[0] + direction[1] * axis_direction[1];
    let reflected = [
        2.0 * projection * axis_direction[0] - direction[0],
        2.0 * projection * axis_direction[1] - direction[1],
    ];
    finite_pair(reflected, "mirrored branch direction")?;
    Ok(reflected)
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
        | DocumentDimensionDefinition::OrientedAngle { target, .. }
        | DocumentDimensionDefinition::SupportingLineOffset { target, .. }
        | DocumentDimensionDefinition::ExactTranslatedSegmentOffset { target, .. }
        | DocumentDimensionDefinition::ProfileOffset { target, .. } => *target,
    }
}

const fn dimension_parameter_kind(
    definition: &DocumentDimensionDefinition,
) -> DocumentParameterKind {
    match definition {
        DocumentDimensionDefinition::OrientedAngle { .. } => DocumentParameterKind::Angle,
        DocumentDimensionDefinition::PointDistance { .. }
        | DocumentDimensionDefinition::CurveLength { .. }
        | DocumentDimensionDefinition::Radius { .. }
        | DocumentDimensionDefinition::Diameter { .. }
        | DocumentDimensionDefinition::SupportingLineOffset { .. }
        | DocumentDimensionDefinition::ExactTranslatedSegmentOffset { .. }
        | DocumentDimensionDefinition::ProfileOffset { .. } => DocumentParameterKind::Length,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProfileOffsetCurveFamily {
    Line,
    CircularArc,
    Circle,
}

const fn profile_offset_endpoint_parameter(traversal: DocumentOffsetTraversal, start: bool) -> f64 {
    match (traversal, start) {
        (DocumentOffsetTraversal::Forward, true) | (DocumentOffsetTraversal::Reverse, false) => 0.0,
        (DocumentOffsetTraversal::Forward, false) | (DocumentOffsetTraversal::Reverse, true) => 1.0,
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

const fn fillet_side_sign(side: DocumentCurveNormalSide) -> f64 {
    match side {
        DocumentCurveNormalSide::Left => 1.0,
        DocumentCurveNormalSide::Right => -1.0,
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

fn rational_weighted_middle_preserving_control(
    middle: [f64; 2],
    weight: f64,
) -> Result<[f64; 2], DocumentError> {
    let weighted_middle = [middle[0] * weight, middle[1] * weight];
    finite_pair(weighted_middle, "rational homogeneous middle")?;
    for (ordinary, weighted) in middle.into_iter().zip(weighted_middle) {
        let recovered = weighted / weight;
        let round_trip_error = (recovered - ordinary).abs();
        let round_trip_tolerance = 64.0 * f64::EPSILON * ordinary.abs();
        if !recovered.is_finite() || round_trip_error > round_trip_tolerance {
            return invalid(
                "rational homogeneous middle",
                "requested Euclidean control loses material precision at this weight",
            );
        }
    }
    Ok(weighted_middle)
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

fn charge_document_item(
    controller: &mut Option<&mut OperationController>,
    counter: OperationWorkCounter,
    checkpoint: OperationCheckpoint,
) -> bool {
    controller
        .as_deref_mut()
        .is_none_or(|controller| controller.charge(counter, 1, checkpoint).is_ok())
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

fn cross(first: [f64; 2], second: [f64; 2]) -> f64 {
    first[0] * second[1] - first[1] * second[0]
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

fn invalid_error(field: &'static str, message: impl Into<String>) -> DocumentError {
    DocumentError::InvalidField {
        field,
        message: message.into(),
    }
}

fn invalid<T>(field: &'static str, message: impl Into<String>) -> Result<T, DocumentError> {
    Err(invalid_error(field, message))
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
        DocumentObjectId::Parameter(id) => id.0,
        DocumentObjectId::ExternalBinding(id) => id.0,
    }
}
