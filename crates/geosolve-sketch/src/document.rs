use std::collections::BTreeSet;
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
}

/// One persistent curve entity.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DesignCurve {
    pub id: CurveId,
    pub label: String,
    pub definition: CurveDefinition,
}

/// Semantic selection of one directed segment within a line or polyline.
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
    FixedCurveLocation {
        contact: ContactId,
    },
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

    /// Evaluates one accepted curve span at an arbitrary rendering/query parameter.
    ///
    /// Line and polyline spans, arcs, and Beziers use `[0, 1]`; circles use an unwrapped angle.
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
            CurveDefinition::Circle { .. } => ContactDomain::Periodic {
                period: std::f64::consts::TAU,
            },
            _ => ContactDomain::Bounded {
                lower: 0.0,
                upper: 1.0,
            },
        };
        self.evaluate_curve_jet_in_domain(span, parameter, domain)
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
                if matches!(value.definition, CurveDefinition::Circle { .. }) {
                    return invalid("feature endpoint", "a periodic circle has no endpoint");
                }
            }
            FeatureRef::CurveAxis { curve } => {
                let value = self.curve(curve).ok_or_else(|| unknown("curve", curve.0))?;
                if !matches!(
                    value.definition,
                    CurveDefinition::Line { .. } | CurveDefinition::Polyline { .. }
                ) {
                    return invalid("feature axis", "axis requires a line or polyline");
                }
            }
            FeatureRef::CurveCenter { curve } => {
                self.require_radial_curve(curve)?;
            }
            FeatureRef::CurveControl { curve, index } => {
                let entity = self.curve(curve).ok_or_else(|| unknown("curve", curve.0))?;
                let count = match &entity.definition {
                    CurveDefinition::Line { .. } => 2,
                    CurveDefinition::Polyline { points, .. } => points.len(),
                    CurveDefinition::Circle { .. } | CurveDefinition::CircularArc { .. } => 0,
                    CurveDefinition::QuadraticBezier { .. } => 3,
                    CurveDefinition::CubicBezier { .. } => 4,
                };
                if usize::try_from(index).map_or(true, |value| value >= count) {
                    return invalid("feature control", "control index is outside the curve");
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
    /// Circles use an angular periodic scalar; every other alpha curve span uses a bounded
    /// `[0, 1]` parameter. Neighborhood, winding, and tangent orientation remain explicit input.
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
        let (unit, scalar_domain, contact_domain) =
            if matches!(definition, CurveDefinition::Circle { .. }) {
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
        if matches!(definition, CurveDefinition::Circle { .. }) {
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

    /// Replaces a circular arc's explicit sweep branch.
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
        let CurveDefinition::CircularArc { sweep: current, .. } = &mut value.definition else {
            return invalid("curve", "sweep edit requires a circular arc");
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
            DocumentObjectId::Curve(id) => retain_remove(&mut candidate.curves, |v| v.id == id)
                .then_some(())
                .ok_or_else(|| unknown("curve", id.0))?,
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
                let owned_scalars = match &candidate
                    .curve(id)
                    .ok_or_else(|| unknown("curve", id.0))?
                    .definition
                {
                    CurveDefinition::Circle { radius, .. } => vec![*radius],
                    CurveDefinition::CircularArc {
                        radius,
                        start_angle,
                        end_angle,
                        ..
                    } => vec![*radius, *start_angle, *end_angle],
                    CurveDefinition::Line { .. }
                    | CurveDefinition::Polyline { .. }
                    | CurveDefinition::QuadraticBezier { .. }
                    | CurveDefinition::CubicBezier { .. } => Vec::new(),
                };
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
            validate_contact(contact, scalar)?;
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

    pub(crate) fn contact_mut(&mut self, id: ContactId) -> Option<&mut ContactSlot> {
        self.contacts.iter_mut().find(|value| value.id == id)
    }

    pub(crate) const fn allocator_cursor(&self) -> PersistentId {
        self.next_id
    }

    pub(crate) fn advance_allocator(&mut self, cursor: PersistentId) {
        if cursor > self.next_id {
            self.next_id = cursor;
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
        }
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
        let count = curve_segment_count(&curve.definition);
        if usize::try_from(span.segment).map_or(true, |index| index >= count) {
            return invalid("curve span", "segment index is outside the curve");
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
        let curve = self
            .curve(contact.curve.curve)
            .ok_or_else(|| unknown("curve", contact.curve.curve.0))?;
        let parameter = contact_total_value(contact, self.require_scalar(contact.parameter)?.value);
        let tangent = match &curve.definition {
            CurveDefinition::Line { start, end, .. } => point_difference(
                self.require_point(*start)?.position,
                self.require_point(*end)?.position,
            ),
            CurveDefinition::Polyline { points, closed, .. } => {
                let index = contact.curve.segment as usize;
                let next = if index + 1 == points.len() {
                    if *closed {
                        0
                    } else {
                        return invalid("contact.curve", "polyline contact segment is invalid");
                    }
                } else {
                    index + 1
                };
                point_difference(
                    self.require_point(points[index])?.position,
                    self.require_point(points[next])?.position,
                )
            }
            CurveDefinition::Circle { radius, .. } => {
                let radius = self.require_scalar(*radius)?.value;
                [-radius * parameter.sin(), radius * parameter.cos()]
            }
            CurveDefinition::CircularArc {
                radius,
                start_angle,
                end_angle,
                sweep,
                ..
            } => {
                let radius = self.require_scalar(*radius)?.value;
                let start = self.require_scalar(*start_angle)?.value;
                let end = self.require_scalar(*end_angle)?.value;
                let signed_sweep = document_arc_signed_sweep(start, end, *sweep)?;
                let angle = start + signed_sweep * parameter;
                [
                    -radius * signed_sweep * angle.sin(),
                    radius * signed_sweep * angle.cos(),
                ]
            }
            CurveDefinition::QuadraticBezier { controls } => {
                let jet = geosolve_geometry::quadratic_bezier_jet(
                    controls.map(|control| {
                        let point = self
                            .point(control)
                            .expect("validated Bezier control reference");
                        geosolve_geometry::Point2::new(point.position[0], point.position[1])
                    }),
                    parameter,
                )
                .map_err(|error| contact_evaluation_error(contact.id, error))?;
                [jet.first_derivative.x, jet.first_derivative.y]
            }
            CurveDefinition::CubicBezier { controls } => {
                let jet = geosolve_geometry::cubic_bezier_jet(
                    controls.map(|control| {
                        let point = self
                            .point(control)
                            .expect("validated Bezier control reference");
                        geosolve_geometry::Point2::new(point.position[0], point.position[1])
                    }),
                    parameter,
                )
                .map_err(|error| contact_evaluation_error(contact.id, error))?;
                [jet.first_derivative.x, jet.first_derivative.y]
            }
        };
        validate_direction(tangent, "contact tangent")?;
        Ok(tangent)
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

fn validate_contact(contact: &ContactSlot, scalar: &DesignScalar) -> Result<(), DocumentError> {
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
            if contact.winding != 0 {
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

fn contact_evaluation_error(
    contact: ContactId,
    error: geosolve_geometry::CurveEvaluationError,
) -> DocumentError {
    match error {
        geosolve_geometry::CurveEvaluationError::Regularity(source) => {
            DocumentError::ContactRegularity { contact, source }
        }
        other => DocumentError::InvalidField {
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
            | CurveDefinition::CubicBezier { .. },
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
        (CurveDefinition::Circle { .. }, ContactDomain::Periodic { period })
            if period.to_bits() == std::f64::consts::TAU.to_bits() =>
        {
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
        DocumentConstraintDefinition::LineCurveTangency { curve_contact, .. } => {
            vec![*curve_contact]
        }
        DocumentConstraintDefinition::CurveCurveContact {
            first_contact,
            second_contact,
        }
        | DocumentConstraintDefinition::CurveCurveTangency {
            first_contact,
            second_contact,
        } => vec![*first_contact, *second_contact],
        _ => Vec::new(),
    }
}

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
            | DocumentConstraintDefinition::LineCurveTangency { line, .. },
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
        | CurveDefinition::CubicBezier { .. } => 1,
    }
}

fn curve_scalars(definition: &CurveDefinition) -> Vec<DesignScalarId> {
    match definition {
        CurveDefinition::Line { .. }
        | CurveDefinition::Polyline { .. }
        | CurveDefinition::QuadraticBezier { .. }
        | CurveDefinition::CubicBezier { .. } => Vec::new(),
        CurveDefinition::Circle { radius, .. } => vec![*radius],
        CurveDefinition::CircularArc {
            radius,
            start_angle,
            end_angle,
            ..
        } => vec![*radius, *start_angle, *end_angle],
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

fn document_arc_signed_sweep(
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
