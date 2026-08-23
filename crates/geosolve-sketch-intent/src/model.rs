// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use crate::ids::{
    CellId, ChildId, ComponentIdentity, ContentDigest, ExternalInputRevision, IntentKey, NodeId,
    PortId, ReservationId, Revision, digest_bytes,
};

/// Maximum exact byte length retained for one external-input or host-artifact
/// component.
pub const MAX_INTENT_OPAQUE_COMPONENT_BYTES: usize = 16 * 1024 * 1024;
/// Maximum variable-cardinality children owned by one declaration.
pub const MAX_INTENT_NODE_CHILDREN: usize = 4_096;
/// Maximum explicit typed inputs owned by one declaration.
pub const MAX_INTENT_NODE_INPUTS: usize = 4_096;
/// Maximum definition literals owned by one declaration.
pub const MAX_INTENT_NODE_FIELDS: usize = 4_096;

/// Invalid typed value or bounded opaque component.
#[derive(Clone, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum IntentModelError {
    #[error("{component} exceeds {limit} bytes")]
    ComponentTooLarge {
        component: &'static str,
        limit: usize,
    },
    #[error("intent literal must be finite")]
    NonFiniteLiteral,
}

/// Cartesian component used by typed writable leaves.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    X,
    Y,
}

/// Unit tag for scalar literals. There is deliberately no expression variant.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentUnit {
    Length,
    Angle,
    Dimensionless,
}

/// Closed data-only value surface used by definitions and instance leaves.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum IntentLiteral {
    Boolean(bool),
    Integer(i64),
    Natural(u64),
    Text(IntentKey),
    Enum(IntentKey),
    Point([f64; 2]),
    Quantity { value: f64, unit: IntentUnit },
}

impl IntentLiteral {
    pub(crate) fn validate(&self) -> Result<(), IntentModelError> {
        match self {
            Self::Point([x, y]) if !x.is_finite() || !y.is_finite() => {
                Err(IntentModelError::NonFiniteLiteral)
            }
            Self::Quantity { value, .. } if !value.is_finite() => {
                Err(IntentModelError::NonFiniteLiteral)
            }
            Self::Boolean(_)
            | Self::Integer(_)
            | Self::Natural(_)
            | Self::Text(_)
            | Self::Enum(_)
            | Self::Point(_)
            | Self::Quantity { .. } => Ok(()),
        }
    }
}

/// Complete existing M78 geometry-authoring recipe catalog.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometryRecipeKind {
    SketchPoint,
    Segment,
    Polyline,
    MidpointLine,
    TwoPointAlignedRectangle,
    ThreePointCornerRectangle,
    CenterRectangle,
    ThreePointCenterRectangle,
    CenterRadiusCircle,
    TwoPointDiameterCircle,
    ThreePointCircle,
    CenterArc,
    ThreePointArc,
    TangentArc,
    CenterAxesEllipse,
    AxisEndpointsEllipse,
    CenterAxesEllipticalArc,
    AxisEndpointsEllipticalArc,
    QuadraticBezier,
    CubicBezier,
    RationalQuadraticConic,
    Parabola,
    Hyperbola,
    OpenControlNurbs,
    PeriodicControlNurbs,
}

impl GeometryRecipeKind {
    pub const ALL: [Self; 25] = [
        Self::SketchPoint,
        Self::Segment,
        Self::Polyline,
        Self::MidpointLine,
        Self::TwoPointAlignedRectangle,
        Self::ThreePointCornerRectangle,
        Self::CenterRectangle,
        Self::ThreePointCenterRectangle,
        Self::CenterRadiusCircle,
        Self::TwoPointDiameterCircle,
        Self::ThreePointCircle,
        Self::CenterArc,
        Self::ThreePointArc,
        Self::TangentArc,
        Self::CenterAxesEllipse,
        Self::AxisEndpointsEllipse,
        Self::CenterAxesEllipticalArc,
        Self::AxisEndpointsEllipticalArc,
        Self::QuadraticBezier,
        Self::CubicBezier,
        Self::RationalQuadraticConic,
        Self::Parabola,
        Self::Hyperbola,
        Self::OpenControlNurbs,
        Self::PeriodicControlNurbs,
    ];

    #[must_use]
    pub const fn child_schema(self) -> IntentChildSchema {
        match self {
            Self::Polyline => IntentChildSchema::PolylineVertex,
            Self::OpenControlNurbs | Self::PeriodicControlNurbs => IntentChildSchema::SplineControl,
            _ => IntentChildSchema::None,
        }
    }
}

/// Complete existing persistent geometric relation catalog.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintKind {
    FixedPoint,
    FixedCoordinate,
    CoincidentWithOrigin,
    PointOnDatumAxis,
    Coincident,
    ExternalPointCoincident,
    Horizontal,
    Vertical,
    HorizontalPoints,
    VerticalPoints,
    HorizontalPointToMidpoint,
    VerticalPointToMidpoint,
    PointOnCurve,
    Parallel,
    Perpendicular,
    ExternalLineCollinear,
    CollinearWithDatumAxis,
    Concentric,
    Collinear,
    EqualLength,
    EqualRadius,
    Midpoint,
    SymmetricAboutLine,
    SymmetricAboutDatumAxis,
    LineCircleTangency,
    CircleCircleTangency,
    CircleArcTangency,
    LineCurveTangency,
    CurveCurveContact,
    CurveCurveTangency,
    CurveDirection,
    EqualCurvature,
    EndpointContinuity,
    LineLineFillet,
    CurveCurveFillet,
}

impl ConstraintKind {
    /// Complete persistent relation inventory owned by the closed intent
    /// schema. Keeping this beside the enum makes omissions compile-visible to
    /// the inventory tests and projection generators.
    pub const ALL: [Self; 35] = [
        Self::FixedPoint,
        Self::FixedCoordinate,
        Self::CoincidentWithOrigin,
        Self::PointOnDatumAxis,
        Self::Coincident,
        Self::ExternalPointCoincident,
        Self::Horizontal,
        Self::Vertical,
        Self::HorizontalPoints,
        Self::VerticalPoints,
        Self::HorizontalPointToMidpoint,
        Self::VerticalPointToMidpoint,
        Self::PointOnCurve,
        Self::Parallel,
        Self::Perpendicular,
        Self::ExternalLineCollinear,
        Self::CollinearWithDatumAxis,
        Self::Concentric,
        Self::Collinear,
        Self::EqualLength,
        Self::EqualRadius,
        Self::Midpoint,
        Self::SymmetricAboutLine,
        Self::SymmetricAboutDatumAxis,
        Self::LineCircleTangency,
        Self::CircleCircleTangency,
        Self::CircleArcTangency,
        Self::LineCurveTangency,
        Self::CurveCurveContact,
        Self::CurveCurveTangency,
        Self::CurveDirection,
        Self::EqualCurvature,
        Self::EndpointContinuity,
        Self::LineLineFillet,
        Self::CurveCurveFillet,
    ];
}

/// Complete current persistent dimension catalog. These are existing native
/// dimensions, not expression or formula constraints.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DimensionKind {
    PointDistance,
    CurveLength,
    Radius,
    Diameter,
    OrientedAngle,
    SupportingLineOffset,
    ExactTranslatedSegmentOffset,
    ProfileOffset,
}

impl DimensionKind {
    pub const ALL: [Self; 8] = [
        Self::PointDistance,
        Self::CurveLength,
        Self::Radius,
        Self::Diameter,
        Self::OrientedAngle,
        Self::SupportingLineOffset,
        Self::ExactTranslatedSegmentOffset,
        Self::ProfileOffset,
    ];
}

/// Complete existing equation-free sketch-operation catalog.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    Split,
    Break,
    Trim,
    Extend,
    Mirror,
    Chamfer,
    AssociativeFillet,
    Rectangle,
    RegularPolygon,
    Slot,
    LinearPattern,
    ProfileOffset,
}

impl OperationKind {
    pub const ALL: [Self; 12] = [
        Self::Split,
        Self::Break,
        Self::Trim,
        Self::Extend,
        Self::Mirror,
        Self::Chamfer,
        Self::AssociativeFillet,
        Self::Rectangle,
        Self::RegularPolygon,
        Self::Slot,
        Self::LinearPattern,
        Self::ProfileOffset,
    ];
}

/// Existing computed feature catalog.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputedFeatureKind {
    FilletSet,
}

impl ComputedFeatureKind {
    pub const ALL: [Self; 1] = [Self::FilletSet];
}

/// Existing host parameter/binding/output declaration categories.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterIntentKind {
    Parameter,
    Binding,
    Output,
}

impl ParameterIntentKind {
    pub const ALL: [Self; 3] = [Self::Parameter, Self::Binding, Self::Output];
}

/// Existing external-reference declaration categories.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalIntentKind {
    Binding,
    SnapshotReference,
}

impl ExternalIntentKind {
    pub const ALL: [Self; 2] = [Self::Binding, Self::SnapshotReference];
}

/// Closed per-object inventory used to normalize an already-materialized flat
/// sketch during bootstrap/ejection. This deliberately does not admit one
/// fictional aggregate "imported baseline" action.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapNativeKind {
    Document,
    Point,
    Scalar,
    Curve,
    Contact,
    Constraint,
    Dimension,
    Parameter,
    ExternalBinding,
    SemanticCatalog,
    SemanticSource,
    CurveTrimView,
    GeometryRole,
    ParameterBinding,
    ParameterOutput,
    ComputedFeature,
    AnnotationPlacement,
}

impl BootstrapNativeKind {
    /// Complete normalized flat-document object inventory.
    pub const ALL: [Self; 17] = [
        Self::Document,
        Self::Point,
        Self::Scalar,
        Self::Curve,
        Self::Contact,
        Self::Constraint,
        Self::Dimension,
        Self::Parameter,
        Self::ExternalBinding,
        Self::SemanticCatalog,
        Self::SemanticSource,
        Self::CurveTrimView,
        Self::GeometryRole,
        Self::ParameterBinding,
        Self::ParameterOutput,
        Self::ComputedFeature,
        Self::AnnotationPlacement,
    ];

    const fn accepts_input(self, role: InputRole) -> bool {
        match self {
            Self::Document | Self::Point | Self::Scalar | Self::SemanticCatalog => false,
            Self::Curve => matches!(
                role,
                InputRole::Point | InputRole::Scalar | InputRole::External
            ),
            Self::Contact => matches!(
                role,
                InputRole::Point | InputRole::Curve | InputRole::Span | InputRole::Scalar
            ),
            Self::Constraint => matches!(
                role,
                InputRole::Point
                    | InputRole::Contact
                    | InputRole::Curve
                    | InputRole::Span
                    | InputRole::Scalar
                    | InputRole::External
                    | InputRole::Source
            ),
            Self::Dimension => matches!(
                role,
                InputRole::Point
                    | InputRole::Curve
                    | InputRole::Span
                    | InputRole::Scalar
                    | InputRole::Profile
                    | InputRole::Chain
            ),
            Self::Parameter => matches!(
                role,
                InputRole::Scalar | InputRole::Dimension | InputRole::Parameter | InputRole::Source
            ),
            Self::ExternalBinding => {
                matches!(role, InputRole::External | InputRole::Source)
            }
            Self::SemanticSource => matches!(
                role,
                InputRole::Catalog | InputRole::Scalar | InputRole::Parameter | InputRole::Source
            ),
            Self::CurveTrimView | Self::GeometryRole => {
                matches!(role, InputRole::Curve | InputRole::Span)
            }
            Self::ParameterBinding | Self::ParameterOutput => matches!(
                role,
                InputRole::Parameter | InputRole::Dimension | InputRole::Scalar
            ),
            Self::ComputedFeature => matches!(
                role,
                InputRole::Point | InputRole::Curve | InputRole::Span | InputRole::Feature
            ),
            Self::AnnotationPlacement => matches!(
                role,
                InputRole::Constraint | InputRole::Dimension | InputRole::Point | InputRole::Curve
            ),
        }
    }
}

/// One exact versioned public-domain object payload used by a normalized
/// bootstrap declaration. The host materializer owns codec interpretation;
/// the intent crate owns bounds, identity, dependencies, and transaction
/// authority.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentBootstrapObject {
    pub kind: BootstrapNativeKind,
    pub codec: IntentKey,
    pub payload: Vec<u8>,
}

impl IntentBootstrapObject {
    /// Constructs one bounded normalized object payload.
    ///
    /// # Errors
    ///
    /// Returns an error when the payload exceeds the opaque-component bound.
    pub fn new(
        kind: BootstrapNativeKind,
        codec: IntentKey,
        payload: Vec<u8>,
    ) -> Result<Self, IntentModelError> {
        validate_component("bootstrap object", &payload)?;
        Ok(Self {
            kind,
            codec,
            payload,
        })
    }

    pub(crate) fn validate(&self) -> Result<(), IntentModelError> {
        validate_component("bootstrap object", &self.payload)
    }
}

/// Explicit identity transition used by the materializer around operations
/// that create, alias, continue, or retire stable topology identity.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityTransitionKind {
    Alias,
    Continue,
    Retire,
}

/// Closed declaration schemas. Family-specific scalar and branch values live
/// in typed literal fields; dependencies remain typed ports.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "family", rename_all = "snake_case", deny_unknown_fields)]
pub enum IntentNodeKind {
    Geometry {
        recipe: GeometryRecipeKind,
    },
    Constraint {
        constraint: ConstraintKind,
    },
    Dimension {
        dimension: DimensionKind,
    },
    Operation {
        operation: OperationKind,
    },
    ComputedFeature {
        feature: ComputedFeatureKind,
    },
    Parameter {
        parameter: ParameterIntentKind,
    },
    External {
        external: ExternalIntentKind,
    },
    /// Honest per-native-object bootstrap/ejection declaration. A host lowers
    /// its versioned payload from [`crate::IntentCandidate`] and remains
    /// responsible for validating the resulting flat scene.
    Bootstrap {
        object: IntentBootstrapObject,
    },
    Annotation,
    Identity {
        transition: IdentityTransitionKind,
        port_kind: IntentPortKind,
    },
}

impl IntentNodeKind {
    #[must_use]
    pub const fn child_schema(&self) -> IntentChildSchema {
        match self {
            Self::Geometry { recipe } => recipe.child_schema(),
            Self::ComputedFeature {
                feature: ComputedFeatureKind::FilletSet,
            } => IntentChildSchema::FilletCorner,
            Self::Operation {
                operation: OperationKind::LinearPattern,
            } => IntentChildSchema::PatternInstance,
            _ => IntentChildSchema::None,
        }
    }

    pub(crate) fn accepts_input(&self, role: InputRole) -> bool {
        match self {
            Self::Geometry {
                recipe: GeometryRecipeKind::TangentArc,
            } => matches!(role, InputRole::Curve | InputRole::Span | InputRole::Point),
            Self::Geometry { .. } => matches!(role, InputRole::Point | InputRole::External),
            Self::Constraint { .. } => matches!(
                role,
                InputRole::Point
                    | InputRole::Curve
                    | InputRole::Span
                    | InputRole::Scalar
                    | InputRole::External
                    | InputRole::Source
            ),
            Self::Dimension { .. } => matches!(
                role,
                InputRole::Point
                    | InputRole::Curve
                    | InputRole::Span
                    | InputRole::Scalar
                    | InputRole::Profile
                    | InputRole::Chain
            ),
            Self::Operation { .. } => matches!(
                role,
                InputRole::Point
                    | InputRole::Curve
                    | InputRole::Span
                    | InputRole::Profile
                    | InputRole::Chain
                    | InputRole::Feature
            ),
            Self::ComputedFeature { .. } => matches!(
                role,
                InputRole::Point | InputRole::Curve | InputRole::Span | InputRole::Feature
            ),
            Self::Parameter { .. } => matches!(
                role,
                InputRole::Scalar | InputRole::Dimension | InputRole::Parameter | InputRole::Source
            ),
            Self::External { .. } => matches!(role, InputRole::External | InputRole::Source),
            Self::Bootstrap { object } => object.kind.accepts_input(role),
            Self::Annotation => matches!(
                role,
                InputRole::Constraint | InputRole::Dimension | InputRole::Point | InputRole::Curve
            ),
            Self::Identity { .. } => role == InputRole::Identity,
        }
    }
}

/// Typed dependency role. The index in [`InputSlot`] distinguishes repeated
/// operands without inventing stringly typed input schemas.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputRole {
    Point,
    Contact,
    Curve,
    Span,
    Scalar,
    Constraint,
    Dimension,
    Feature,
    Profile,
    Chain,
    Parameter,
    External,
    Source,
    Catalog,
    Identity,
}

impl InputRole {
    #[must_use]
    pub const fn expected_kind(self) -> Option<IntentPortKind> {
        match self {
            Self::Point => Some(IntentPortKind::Point),
            Self::Contact => Some(IntentPortKind::Contact),
            Self::Curve => Some(IntentPortKind::Curve),
            Self::Span => Some(IntentPortKind::CurveSpan),
            Self::Scalar => Some(IntentPortKind::Scalar),
            Self::Constraint => Some(IntentPortKind::Constraint),
            Self::Dimension => Some(IntentPortKind::Dimension),
            Self::Feature => Some(IntentPortKind::Feature),
            Self::Profile => Some(IntentPortKind::Profile),
            Self::Chain => Some(IntentPortKind::Chain),
            Self::Parameter => Some(IntentPortKind::Parameter),
            Self::External => Some(IntentPortKind::ExternalBinding),
            Self::Source => Some(IntentPortKind::Source),
            Self::Catalog => Some(IntentPortKind::SemanticCatalog),
            Self::Identity => None,
        }
    }
}

/// Stable indexed typed dependency slot.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InputSlot {
    pub role: InputRole,
    pub index: u16,
}

impl fmt::Display for InputSlot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{:04x}",
            input_role_key(self.role),
            self.index
        )
    }
}

impl FromStr for InputSlot {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (role, index) = value
            .split_once(':')
            .ok_or_else(|| "invalid input slot".to_owned())?;
        Ok(Self {
            role: parse_input_role(role).ok_or_else(|| "invalid input role".to_owned())?,
            index: u16::from_str_radix(index, 16).map_err(|_| "invalid input index".to_owned())?,
        })
    }
}

impl Serialize for InputSlot {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for InputSlot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

impl InputSlot {
    #[must_use]
    pub const fn new(role: InputRole, index: u16) -> Self {
        Self { role, index }
    }
}

/// Semantic kind of one stable port and its reserved native identity.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentPortKind {
    Point,
    /// Authored or derived Cartesian handle retained by intent but not
    /// materialized as a persistent `DesignPointId`.
    HandlePoint,
    Scalar,
    Curve,
    CurveSpan,
    Contact,
    Constraint,
    Dimension,
    Source,
    Parameter,
    ParameterBinding,
    ParameterOutput,
    ExternalBinding,
    SemanticCatalog,
    Profile,
    Chain,
    Operation,
    Feature,
    FeatureCorner,
    Annotation,
    Collection,
}

/// Persistent sketch identity allocated when one native output is
/// materialized. Logical-only ports deliberately have no value in this enum.
/// Constraint and dimension sources are distinct so their required atomic
/// owner/source pairing remains auditable.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentNativeReservationKind {
    Point,
    Scalar,
    Curve,
    Contact,
    Constraint,
    ConstraintSource,
    Dimension,
    DimensionSource,
    Parameter,
    ExternalBinding,
    SemanticCatalog,
    SemanticSource,
}

impl IntentNativeReservationKind {
    #[must_use]
    pub const fn port_kind(self) -> IntentPortKind {
        match self {
            Self::Point => IntentPortKind::Point,
            Self::Scalar => IntentPortKind::Scalar,
            Self::Curve => IntentPortKind::Curve,
            Self::Contact => IntentPortKind::Contact,
            Self::Constraint => IntentPortKind::Constraint,
            Self::ConstraintSource | Self::DimensionSource | Self::SemanticSource => {
                IntentPortKind::Source
            }
            Self::Dimension => IntentPortKind::Dimension,
            Self::Parameter => IntentPortKind::Parameter,
            Self::ExternalBinding => IntentPortKind::ExternalBinding,
            Self::SemanticCatalog => IntentPortKind::SemanticCatalog,
        }
    }
}

/// Semantic port role generated by a closed node schema.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentPortRole {
    Primary,
    Start,
    End,
    Center,
    Midpoint,
    Corner,
    Control,
    MajorAxisPoint,
    MinorAxisPoint,
    Curve,
    Span,
    Contact,
    Target,
    Constraint,
    Dimension,
    Source,
    Catalog,
    Operation,
    Feature,
    FeatureCorner,
    Parameter,
    Binding,
    Output,
    External,
    Annotation,
    Collection,
    Profile,
    Chain,
    Result,
}

/// Semantic selector for a schema-generated port. Child ordinals are accepted
/// only while planning a create-node transaction and resolve to stable IDs.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IntentPortSelector {
    Node {
        role: IntentPortRole,
        index: u16,
    },
    InitialChild {
        ordinal: u16,
        role: IntentPortRole,
        index: u16,
    },
}

impl fmt::Display for IntentPortSelector {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Node { role, index } => {
                write!(formatter, "node:{}:{index:04x}", port_role_key(*role))
            }
            Self::InitialChild {
                ordinal,
                role,
                index,
            } => write!(
                formatter,
                "child:{ordinal:04x}:{}:{index:04x}",
                port_role_key(*role)
            ),
        }
    }
}

impl FromStr for IntentPortSelector {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let parts = value.split(':').collect::<Vec<_>>();
        match parts.as_slice() {
            ["node", role, index] => Ok(Self::Node {
                role: parse_port_role(role).ok_or_else(|| "invalid port role".to_owned())?,
                index: u16::from_str_radix(index, 16)
                    .map_err(|_| "invalid port index".to_owned())?,
            }),
            ["child", ordinal, role, index] => Ok(Self::InitialChild {
                ordinal: u16::from_str_radix(ordinal, 16)
                    .map_err(|_| "invalid child ordinal".to_owned())?,
                role: parse_port_role(role).ok_or_else(|| "invalid port role".to_owned())?,
                index: u16::from_str_radix(index, 16)
                    .map_err(|_| "invalid port index".to_owned())?,
            }),
            _ => Err("invalid port selector".to_owned()),
        }
    }
}

impl Serialize for IntentPortSelector {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for IntentPortSelector {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

/// Stable typed reference to one logical output.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentPortRef {
    pub node: NodeId,
    pub port: PortId,
    pub kind: IntentPortKind,
}

/// A port reference in an unordered patch. Transaction aliases permit atomic
/// forward references without exposing allocator IDs.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "source", rename_all = "snake_case", deny_unknown_fields)]
pub enum PatchPortRef {
    Stable {
        port: IntentPortRef,
    },
    Alias {
        node: IntentKey,
        selector: IntentPortSelector,
    },
}

/// Explicit stable identity flow. Only `Continued` and `Retired` consume a
/// generation; aliases remain non-owning views of the same token.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum IntentIdentityFlow {
    /// Stable graph identity with no corresponding persistent sketch object.
    OwnedLogical,
    Created {
        reservation: ReservationId,
    },
    Aliased {
        source: IntentPortRef,
    },
    Continued {
        source: IntentPortRef,
        generation: u64,
    },
    Retired {
        source: IntentPortRef,
        generation: u64,
    },
}

/// Variable-cardinality child schema owned by a declaration.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentChildSchema {
    None,
    PolylineVertex,
    SplineControl,
    FilletCorner,
    PatternInstance,
}

/// Definition-field key. Closed node-family enums own semantics while this
/// bounded key keeps version-one family payloads compact and data-only.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct IntentFieldKey(pub IntentKey);

/// Exact writable field generated from a stable output.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LeafField {
    X,
    Y,
    Value,
    Angle,
    Weight,
    Parameter,
}

/// Stable writable numerical leaf owner.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LeafRef {
    pub node: NodeId,
    pub port: PortId,
    pub field: LeafField,
}

impl fmt::Display for LeafRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{}:{}",
            self.node,
            self.port,
            leaf_field_key(self.field)
        )
    }
}

impl FromStr for LeafRef {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let parts = value.split(':').collect::<Vec<_>>();
        let [node, port, field] = parts.as_slice() else {
            return Err("invalid writable leaf".to_owned());
        };
        Ok(Self {
            node: node.parse().map_err(|_| "invalid leaf node".to_owned())?,
            port: port.parse().map_err(|_| "invalid leaf port".to_owned())?,
            field: parse_leaf_field(field).ok_or_else(|| "invalid leaf field".to_owned())?,
        })
    }
}

impl Serialize for LeafRef {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for LeafRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

fn input_role_key(role: InputRole) -> &'static str {
    match role {
        InputRole::Point => "point",
        InputRole::Contact => "contact",
        InputRole::Curve => "curve",
        InputRole::Span => "span",
        InputRole::Scalar => "scalar",
        InputRole::Constraint => "constraint",
        InputRole::Dimension => "dimension",
        InputRole::Feature => "feature",
        InputRole::Profile => "profile",
        InputRole::Chain => "chain",
        InputRole::Parameter => "parameter",
        InputRole::External => "external",
        InputRole::Source => "source",
        InputRole::Catalog => "catalog",
        InputRole::Identity => "identity",
    }
}

fn parse_input_role(value: &str) -> Option<InputRole> {
    Some(match value {
        "point" => InputRole::Point,
        "contact" => InputRole::Contact,
        "curve" => InputRole::Curve,
        "span" => InputRole::Span,
        "scalar" => InputRole::Scalar,
        "constraint" => InputRole::Constraint,
        "dimension" => InputRole::Dimension,
        "feature" => InputRole::Feature,
        "profile" => InputRole::Profile,
        "chain" => InputRole::Chain,
        "parameter" => InputRole::Parameter,
        "external" => InputRole::External,
        "source" => InputRole::Source,
        "catalog" => InputRole::Catalog,
        "identity" => InputRole::Identity,
        _ => return None,
    })
}

fn port_role_key(role: IntentPortRole) -> &'static str {
    match role {
        IntentPortRole::Primary => "primary",
        IntentPortRole::Start => "start",
        IntentPortRole::End => "end",
        IntentPortRole::Center => "center",
        IntentPortRole::Midpoint => "midpoint",
        IntentPortRole::Corner => "corner",
        IntentPortRole::Control => "control",
        IntentPortRole::MajorAxisPoint => "major_axis_point",
        IntentPortRole::MinorAxisPoint => "minor_axis_point",
        IntentPortRole::Curve => "curve",
        IntentPortRole::Span => "span",
        IntentPortRole::Contact => "contact",
        IntentPortRole::Target => "target",
        IntentPortRole::Constraint => "constraint",
        IntentPortRole::Dimension => "dimension",
        IntentPortRole::Source => "source",
        IntentPortRole::Catalog => "catalog",
        IntentPortRole::Operation => "operation",
        IntentPortRole::Feature => "feature",
        IntentPortRole::FeatureCorner => "feature_corner",
        IntentPortRole::Parameter => "parameter",
        IntentPortRole::Binding => "binding",
        IntentPortRole::Output => "output",
        IntentPortRole::External => "external",
        IntentPortRole::Annotation => "annotation",
        IntentPortRole::Collection => "collection",
        IntentPortRole::Profile => "profile",
        IntentPortRole::Chain => "chain",
        IntentPortRole::Result => "result",
    }
}

fn parse_port_role(value: &str) -> Option<IntentPortRole> {
    Some(match value {
        "primary" => IntentPortRole::Primary,
        "start" => IntentPortRole::Start,
        "end" => IntentPortRole::End,
        "center" => IntentPortRole::Center,
        "midpoint" => IntentPortRole::Midpoint,
        "corner" => IntentPortRole::Corner,
        "control" => IntentPortRole::Control,
        "major_axis_point" => IntentPortRole::MajorAxisPoint,
        "minor_axis_point" => IntentPortRole::MinorAxisPoint,
        "curve" => IntentPortRole::Curve,
        "span" => IntentPortRole::Span,
        "contact" => IntentPortRole::Contact,
        "target" => IntentPortRole::Target,
        "constraint" => IntentPortRole::Constraint,
        "dimension" => IntentPortRole::Dimension,
        "source" => IntentPortRole::Source,
        "catalog" => IntentPortRole::Catalog,
        "operation" => IntentPortRole::Operation,
        "feature" => IntentPortRole::Feature,
        "feature_corner" => IntentPortRole::FeatureCorner,
        "parameter" => IntentPortRole::Parameter,
        "binding" => IntentPortRole::Binding,
        "output" => IntentPortRole::Output,
        "external" => IntentPortRole::External,
        "annotation" => IntentPortRole::Annotation,
        "collection" => IntentPortRole::Collection,
        "profile" => IntentPortRole::Profile,
        "chain" => IntentPortRole::Chain,
        "result" => IntentPortRole::Result,
        _ => return None,
    })
}

fn leaf_field_key(field: LeafField) -> &'static str {
    match field {
        LeafField::X => "x",
        LeafField::Y => "y",
        LeafField::Value => "value",
        LeafField::Angle => "angle",
        LeafField::Weight => "weight",
        LeafField::Parameter => "parameter",
    }
}

fn parse_leaf_field(value: &str) -> Option<LeafField> {
    Some(match value {
        "x" => LeafField::X,
        "y" => LeafField::Y,
        "value" => LeafField::Value,
        "angle" => LeafField::Angle,
        "weight" => LeafField::Weight,
        "parameter" => LeafField::Parameter,
        _ => return None,
    })
}

/// Caller-authored semantic draft. Outputs, writable leaves, children, native
/// reservations, stable IDs, and identity generations are generated by Rust.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentNodeDraft {
    pub kind: IntentNodeKind,
    /// Stable unique developer-facing symbol used by source, AI, and host
    /// projections. Display renames never mutate this key.
    pub symbol: IntentKey,
    /// Initial organization-only display name.
    pub name: IntentKey,
    pub inputs: BTreeMap<InputSlot, PatchPortRef>,
    pub fields: BTreeMap<IntentFieldKey, IntentLiteral>,
    pub initial_instance: BTreeMap<IntentPortSelector, BTreeMap<LeafField, IntentLiteral>>,
    pub dynamic_children: u16,
    pub suppressed: bool,
}

impl IntentNodeDraft {
    #[must_use]
    pub fn new(kind: IntentNodeKind, symbol: IntentKey) -> Self {
        Self {
            kind,
            name: symbol.clone(),
            symbol,
            inputs: BTreeMap::new(),
            fields: BTreeMap::new(),
            initial_instance: BTreeMap::new(),
            dynamic_children: 0,
            suppressed: false,
        }
    }

    /// Sets the initial organization-only display name without changing the
    /// durable developer symbol.
    #[must_use]
    pub fn with_display_name(mut self, name: IntentKey) -> Self {
        self.name = name;
        self
    }

    #[must_use]
    pub fn with_input(mut self, slot: InputSlot, source: PatchPortRef) -> Self {
        self.inputs.insert(slot, source);
        self
    }

    #[must_use]
    pub fn with_dynamic_children(mut self, count: u16) -> Self {
        self.dynamic_children = count;
        self
    }

    #[must_use]
    pub fn with_field(mut self, field: IntentFieldKey, value: IntentLiteral) -> Self {
        self.fields.insert(field, value);
        self
    }

    #[must_use]
    pub fn with_instance_leaf(
        mut self,
        port: IntentPortSelector,
        field: LeafField,
        value: IntentLiteral,
    ) -> Self {
        self.initial_instance
            .entry(port)
            .or_default()
            .insert(field, value);
        self
    }
}

/// Exact identity of canonical graph definition state.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentGraphIdentity(pub ComponentIdentity);

/// Exact identity of writable instance state.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentInstanceIdentity(pub ComponentIdentity);

/// Exact identity of the monotonic native-reservation/tombstone ledger.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentReservationLedgerIdentity(pub ComponentIdentity);

/// Exact identity of non-semantic organization state.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentOrganizationIdentity(pub ComponentIdentity);

/// Exact writable instance values. Missing leaves use schema/materializer
/// defaults and remain reconstructible.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentInstanceState {
    pub(crate) revision: Revision,
    pub(crate) values: BTreeMap<LeafRef, IntentLiteral>,
}

impl IntentInstanceState {
    #[must_use]
    pub fn values(&self) -> &BTreeMap<LeafRef, IntentLiteral> {
        &self.values
    }

    /// Returns the exact revision/digest identity of all instance leaves.
    ///
    /// # Panics
    ///
    /// Panics only if serialization of this closed schema fails.
    #[must_use]
    pub fn identity(&self) -> IntentInstanceIdentity {
        IntentInstanceIdentity(ComponentIdentity {
            revision: self.revision,
            digest: digest_bytes(
                &serde_json::to_vec(&(&self.revision, &self.values))
                    .expect("instance state is infallibly serializable"),
            ),
        })
    }

    pub(crate) fn empty() -> Self {
        Self {
            revision: Revision::from_raw(0),
            values: BTreeMap::new(),
        }
    }
}

/// One non-semantic Design-panel cell.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrganizationCell {
    pub id: CellId,
    pub name: IntentKey,
    pub declarations: Vec<NodeId>,
}

/// Presentation-only cells, declaration order, names, and aliases.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentOrganization {
    pub(crate) revision: Revision,
    pub(crate) default_cell: CellId,
    pub(crate) cell_order: Vec<CellId>,
    pub(crate) cells: BTreeMap<CellId, OrganizationCell>,
    pub(crate) node_names: BTreeMap<NodeId, IntentKey>,
}

impl IntentOrganization {
    #[must_use]
    pub const fn default_cell(&self) -> CellId {
        self.default_cell
    }

    #[must_use]
    pub fn cell_order(&self) -> &[CellId] {
        &self.cell_order
    }

    #[must_use]
    pub fn cells(&self) -> &BTreeMap<CellId, OrganizationCell> {
        &self.cells
    }

    #[must_use]
    pub fn node_names(&self) -> &BTreeMap<NodeId, IntentKey> {
        &self.node_names
    }

    /// Returns the exact revision/digest identity of presentation organization.
    ///
    /// # Panics
    ///
    /// Panics only if serialization of this closed schema fails.
    #[must_use]
    pub fn identity(&self) -> IntentOrganizationIdentity {
        IntentOrganizationIdentity(ComponentIdentity {
            revision: self.revision,
            digest: digest_bytes(
                &serde_json::to_vec(&(
                    self.revision,
                    self.default_cell,
                    &self.cell_order,
                    &self.cells,
                    &self.node_names,
                ))
                .expect("organization is infallibly serializable"),
            ),
        })
    }

    pub(crate) fn new(default_cell: CellId, name: IntentKey) -> Self {
        Self {
            revision: Revision::from_raw(0),
            default_cell,
            cell_order: vec![default_cell],
            cells: BTreeMap::from([(
                default_cell,
                OrganizationCell {
                    id: default_cell,
                    name,
                    declarations: Vec::new(),
                },
            )]),
            node_names: BTreeMap::new(),
        }
    }
}

/// Exact host input bytes and their host-owned revision. The intent crate does
/// not reinterpret `ParameterBatch` or external snapshot payloads.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentExternalInputs {
    pub revision: ExternalInputRevision,
    pub parameter_batch: Vec<u8>,
    pub external_snapshots: Vec<u8>,
}

impl IntentExternalInputs {
    /// Constructs one bounded opaque host-input snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error when either opaque component exceeds its bound.
    pub fn new(
        revision: ExternalInputRevision,
        parameter_batch: Vec<u8>,
        external_snapshots: Vec<u8>,
    ) -> Result<Self, IntentModelError> {
        validate_component("parameter batch", &parameter_batch)?;
        validate_component("external snapshots", &external_snapshots)?;
        Ok(Self {
            revision,
            parameter_batch,
            external_snapshots,
        })
    }

    /// Returns the exact host-input identity.
    ///
    /// # Panics
    ///
    /// Panics only if serialization of this closed schema fails.
    #[must_use]
    pub fn identity(&self) -> IntentExternalInputsIdentity {
        IntentExternalInputsIdentity {
            revision: self.revision,
            digest: digest_bytes(
                &serde_json::to_vec(&(
                    self.revision,
                    &self.parameter_batch,
                    &self.external_snapshots,
                ))
                .expect("external inputs are infallibly serializable"),
            ),
        }
    }
}

impl Default for IntentExternalInputs {
    fn default() -> Self {
        Self {
            revision: ExternalInputRevision::from_raw(0),
            parameter_batch: Vec::new(),
            external_snapshots: Vec::new(),
        }
    }
}

/// Exact identity of current host inputs.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentExternalInputsIdentity {
    pub revision: ExternalInputRevision,
    pub digest: ContentDigest,
}

/// Solver-relevant identity. Organization and transaction history are
/// deliberately absent.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentSemanticIdentity {
    pub graph: IntentGraphIdentity,
    pub instance: IntentInstanceIdentity,
    pub reservations: IntentReservationLedgerIdentity,
    pub external_inputs: IntentExternalInputsIdentity,
}

/// Exact bounded materialization, reverse-owner, and validation artifacts
/// supplied by the host for one semantic identity.
///
/// This equation-free crate authenticates these bytes but cannot independently
/// validate native geometry or residuals. The host materializer remains
/// responsible for doing so before returning [`crate::IntentEvaluation::Accepted`].
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializationEvidence {
    pub external_inputs: IntentExternalInputsIdentity,
    pub materialization: Vec<u8>,
    pub ownership: Vec<u8>,
    pub host_validation: Vec<u8>,
    pub digest: ContentDigest,
}

impl MaterializationEvidence {
    /// Constructs bounded, content-authenticated host artifacts.
    ///
    /// # Errors
    ///
    /// Returns an error for an excessive component.
    ///
    /// # Panics
    ///
    /// Panics only if serialization of this closed schema fails.
    pub fn new_host_artifacts(
        external_inputs: IntentExternalInputsIdentity,
        materialization: Vec<u8>,
        ownership: Vec<u8>,
        host_validation: Vec<u8>,
    ) -> Result<Self, IntentModelError> {
        validate_component("materialization", &materialization)?;
        validate_component("ownership map", &ownership)?;
        validate_component("host validation artifact", &host_validation)?;
        let digest = digest_bytes(
            &serde_json::to_vec(&(
                external_inputs,
                &materialization,
                &ownership,
                &host_validation,
            ))
            .expect("materialization evidence is infallibly serializable"),
        );
        Ok(Self {
            external_inputs,
            materialization,
            ownership,
            host_validation,
            digest,
        })
    }
}

fn validate_component(component: &'static str, bytes: &[u8]) -> Result<(), IntentModelError> {
    if bytes.len() > MAX_INTENT_OPAQUE_COMPONENT_BYTES {
        return Err(IntentModelError::ComponentTooLarge {
            component,
            limit: MAX_INTENT_OPAQUE_COMPONENT_BYTES,
        });
    }
    Ok(())
}

/// Internal schema-generated port description.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PortSpec {
    pub selector: IntentPortSelector,
    pub kind: IntentPortKind,
    pub writable: &'static [LeafField],
    pub native: Option<IntentNativeReservationKind>,
    pub alias_input: Option<InputSlot>,
}

const NO_LEAVES: &[LeafField] = &[];
const POINT_LEAVES: &[LeafField] = &[LeafField::X, LeafField::Y];
const VALUE_LEAF: &[LeafField] = &[LeafField::Value];
const ANGLE_LEAF: &[LeafField] = &[LeafField::Angle];
const WEIGHT_LEAF: &[LeafField] = &[LeafField::Weight];
const PARAMETER_LEAF: &[LeafField] = &[LeafField::Parameter];

#[allow(
    clippy::too_many_lines,
    reason = "one closed family table keeps native and logical output storage auditable"
)]
pub(crate) fn node_port_specs(kind: &IntentNodeKind) -> Vec<PortSpec> {
    let mut specs = Vec::new();
    let mut push = |role, index, port_kind, writable, native| {
        specs.push(PortSpec {
            selector: IntentPortSelector::Node { role, index },
            kind: port_kind,
            writable,
            native,
            alias_input: None,
        });
    };
    match kind {
        IntentNodeKind::Geometry { recipe } => return geometry_port_specs(*recipe),
        IntentNodeKind::Constraint { constraint } => {
            for index in 0..constraint_contact_count(*constraint) {
                push(
                    IntentPortRole::Contact,
                    index,
                    IntentPortKind::Contact,
                    NO_LEAVES,
                    Some(IntentNativeReservationKind::Contact),
                );
                push(
                    IntentPortRole::Parameter,
                    index,
                    IntentPortKind::Scalar,
                    PARAMETER_LEAF,
                    Some(IntentNativeReservationKind::Scalar),
                );
            }
            push(
                IntentPortRole::Constraint,
                0,
                IntentPortKind::Constraint,
                NO_LEAVES,
                Some(IntentNativeReservationKind::Constraint),
            );
            push(
                IntentPortRole::Source,
                0,
                IntentPortKind::Source,
                NO_LEAVES,
                Some(IntentNativeReservationKind::ConstraintSource),
            );
        }
        IntentNodeKind::Dimension { .. } => {
            push(
                IntentPortRole::Target,
                0,
                IntentPortKind::Scalar,
                VALUE_LEAF,
                Some(IntentNativeReservationKind::Scalar),
            );
            push(
                IntentPortRole::Dimension,
                0,
                IntentPortKind::Dimension,
                NO_LEAVES,
                Some(IntentNativeReservationKind::Dimension),
            );
            push(
                IntentPortRole::Source,
                0,
                IntentPortKind::Source,
                NO_LEAVES,
                Some(IntentNativeReservationKind::DimensionSource),
            );
        }
        IntentNodeKind::Operation { .. } => {
            push(
                IntentPortRole::Operation,
                0,
                IntentPortKind::Operation,
                NO_LEAVES,
                None,
            );
            push(
                IntentPortRole::Collection,
                0,
                IntentPortKind::Collection,
                NO_LEAVES,
                None,
            );
        }
        IntentNodeKind::ComputedFeature { .. } => push(
            IntentPortRole::Feature,
            0,
            IntentPortKind::Feature,
            NO_LEAVES,
            None,
        ),
        IntentNodeKind::Parameter { parameter } => {
            let (port_kind, native) = match parameter {
                ParameterIntentKind::Parameter => (
                    IntentPortKind::Parameter,
                    Some(IntentNativeReservationKind::Parameter),
                ),
                ParameterIntentKind::Binding => (IntentPortKind::ParameterBinding, None),
                ParameterIntentKind::Output => (IntentPortKind::ParameterOutput, None),
            };
            push(IntentPortRole::Parameter, 0, port_kind, NO_LEAVES, native);
        }
        IntentNodeKind::External { .. } => push(
            IntentPortRole::External,
            0,
            IntentPortKind::ExternalBinding,
            NO_LEAVES,
            Some(IntentNativeReservationKind::ExternalBinding),
        ),
        IntentNodeKind::Bootstrap { object } => {
            use BootstrapNativeKind as B;
            let (role, port_kind, native) = match object.kind {
                B::Document | B::GeometryRole => {
                    (IntentPortRole::Result, IntentPortKind::Collection, None)
                }
                B::Point => (
                    IntentPortRole::Primary,
                    IntentPortKind::Point,
                    Some(IntentNativeReservationKind::Point),
                ),
                B::Scalar => (
                    IntentPortRole::Target,
                    IntentPortKind::Scalar,
                    Some(IntentNativeReservationKind::Scalar),
                ),
                B::Curve => (
                    IntentPortRole::Curve,
                    IntentPortKind::Curve,
                    Some(IntentNativeReservationKind::Curve),
                ),
                B::Contact => (
                    IntentPortRole::Contact,
                    IntentPortKind::Contact,
                    Some(IntentNativeReservationKind::Contact),
                ),
                B::Constraint => (
                    IntentPortRole::Constraint,
                    IntentPortKind::Constraint,
                    Some(IntentNativeReservationKind::Constraint),
                ),
                B::Dimension => (
                    IntentPortRole::Dimension,
                    IntentPortKind::Dimension,
                    Some(IntentNativeReservationKind::Dimension),
                ),
                B::Parameter => (
                    IntentPortRole::Parameter,
                    IntentPortKind::Parameter,
                    Some(IntentNativeReservationKind::Parameter),
                ),
                B::ExternalBinding => (
                    IntentPortRole::External,
                    IntentPortKind::ExternalBinding,
                    Some(IntentNativeReservationKind::ExternalBinding),
                ),
                B::SemanticCatalog => (
                    IntentPortRole::Catalog,
                    IntentPortKind::SemanticCatalog,
                    Some(IntentNativeReservationKind::SemanticCatalog),
                ),
                B::SemanticSource => (
                    IntentPortRole::Source,
                    IntentPortKind::Source,
                    Some(IntentNativeReservationKind::SemanticSource),
                ),
                B::CurveTrimView => (IntentPortRole::Span, IntentPortKind::CurveSpan, None),
                B::ParameterBinding => (
                    IntentPortRole::Binding,
                    IntentPortKind::ParameterBinding,
                    None,
                ),
                B::ParameterOutput => (
                    IntentPortRole::Output,
                    IntentPortKind::ParameterOutput,
                    None,
                ),
                B::ComputedFeature => (IntentPortRole::Feature, IntentPortKind::Feature, None),
                B::AnnotationPlacement => {
                    (IntentPortRole::Annotation, IntentPortKind::Annotation, None)
                }
            };
            let writable = match object.kind {
                B::Point => POINT_LEAVES,
                B::Scalar => VALUE_LEAF,
                _ => NO_LEAVES,
            };
            push(role, 0, port_kind, writable, native);
            if object.kind == B::Curve {
                push(
                    IntentPortRole::Span,
                    0,
                    IntentPortKind::CurveSpan,
                    NO_LEAVES,
                    None,
                );
            }
            if object.kind == B::Constraint {
                push(
                    IntentPortRole::Source,
                    0,
                    IntentPortKind::Source,
                    NO_LEAVES,
                    Some(IntentNativeReservationKind::ConstraintSource),
                );
            } else if object.kind == B::Dimension {
                push(
                    IntentPortRole::Source,
                    0,
                    IntentPortKind::Source,
                    NO_LEAVES,
                    Some(IntentNativeReservationKind::DimensionSource),
                );
            }
        }
        IntentNodeKind::Annotation => push(
            IntentPortRole::Annotation,
            0,
            IntentPortKind::Annotation,
            NO_LEAVES,
            None,
        ),
        IntentNodeKind::Identity { port_kind, .. } => {
            push(IntentPortRole::Result, 0, *port_kind, NO_LEAVES, None);
        }
    }
    specs
}

pub(crate) fn child_port_specs(schema: IntentChildSchema, ordinal: u16) -> Vec<PortSpec> {
    let make = |role, kind, writable, native, alias_input| PortSpec {
        selector: IntentPortSelector::InitialChild {
            ordinal,
            role,
            index: 0,
        },
        kind,
        writable,
        native,
        alias_input,
    };
    match schema {
        IntentChildSchema::None => Vec::new(),
        IntentChildSchema::PolylineVertex => vec![
            make(
                IntentPortRole::Corner,
                IntentPortKind::Point,
                POINT_LEAVES,
                Some(IntentNativeReservationKind::Point),
                Some(InputSlot::new(InputRole::Point, ordinal)),
            ),
            make(
                IntentPortRole::Span,
                IntentPortKind::CurveSpan,
                NO_LEAVES,
                None,
                None,
            ),
        ],
        IntentChildSchema::SplineControl => vec![
            make(
                IntentPortRole::Control,
                IntentPortKind::Point,
                POINT_LEAVES,
                Some(IntentNativeReservationKind::Point),
                Some(InputSlot::new(InputRole::Point, ordinal)),
            ),
            make(
                IntentPortRole::Target,
                IntentPortKind::Scalar,
                WEIGHT_LEAF,
                Some(IntentNativeReservationKind::Scalar),
                None,
            ),
        ],
        IntentChildSchema::FilletCorner => vec![make(
            IntentPortRole::FeatureCorner,
            IntentPortKind::FeatureCorner,
            NO_LEAVES,
            None,
            None,
        )],
        IntentChildSchema::PatternInstance => vec![make(
            IntentPortRole::Result,
            IntentPortKind::Collection,
            NO_LEAVES,
            None,
            None,
        )],
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive table audits persistent native storage for all 25 authoring recipes"
)]
fn geometry_port_specs(recipe: GeometryRecipeKind) -> Vec<PortSpec> {
    use GeometryRecipeKind as G;
    use IntentPortRole as R;

    let specs = std::cell::RefCell::new(Vec::new());
    let point = |role, index, authored_input: Option<u16>| {
        specs.borrow_mut().push(PortSpec {
            selector: IntentPortSelector::Node { role, index },
            kind: IntentPortKind::Point,
            writable: POINT_LEAVES,
            native: Some(IntentNativeReservationKind::Point),
            alias_input: authored_input.map(|index| InputSlot::new(InputRole::Point, index)),
        });
    };
    let handle = |role, index| {
        specs.borrow_mut().push(PortSpec {
            selector: IntentPortSelector::Node { role, index },
            kind: IntentPortKind::HandlePoint,
            writable: NO_LEAVES,
            native: None,
            alias_input: None,
        });
    };
    let scalar = |index, writable| {
        specs.borrow_mut().push(PortSpec {
            selector: IntentPortSelector::Node {
                role: R::Target,
                index,
            },
            kind: IntentPortKind::Scalar,
            writable,
            native: Some(IntentNativeReservationKind::Scalar),
            alias_input: None,
        });
    };

    match recipe {
        G::SketchPoint => point(R::Primary, 0, Some(0)),
        G::Segment => {
            point(R::Start, 0, Some(0));
            point(R::End, 0, Some(1));
        }
        G::Polyline | G::OpenControlNurbs | G::PeriodicControlNurbs => {}
        G::MidpointLine => {
            point(R::Start, 0, None);
            point(R::End, 0, Some(1));
            point(R::Midpoint, 0, Some(0));
        }
        G::TwoPointAlignedRectangle => {
            point(R::Corner, 0, Some(0));
            point(R::Corner, 1, None);
            point(R::Corner, 2, Some(1));
            point(R::Corner, 3, None);
        }
        G::ThreePointCornerRectangle => {
            point(R::Corner, 0, Some(0));
            point(R::Corner, 1, Some(1));
            point(R::Corner, 2, Some(2));
            point(R::Corner, 3, None);
        }
        G::CenterRectangle => {
            point(R::Corner, 0, Some(1));
            point(R::Corner, 1, None);
            point(R::Corner, 2, None);
            point(R::Corner, 3, None);
            point(R::Center, 0, Some(0));
        }
        G::ThreePointCenterRectangle => {
            point(R::Corner, 0, Some(2));
            point(R::Corner, 1, None);
            point(R::Corner, 2, None);
            point(R::Corner, 3, None);
            point(R::Center, 0, Some(0));
            handle(R::Midpoint, 0);
        }
        G::CenterRadiusCircle => {
            point(R::Center, 0, Some(0));
            handle(R::Control, 0);
            scalar(0, VALUE_LEAF);
        }
        G::TwoPointDiameterCircle => {
            handle(R::Start, 0);
            handle(R::End, 0);
            point(R::Center, 0, None);
            scalar(0, VALUE_LEAF);
        }
        G::ThreePointCircle => {
            handle(R::Control, 0);
            handle(R::Control, 1);
            handle(R::Control, 2);
            point(R::Center, 0, None);
            scalar(0, VALUE_LEAF);
        }
        G::CenterArc => {
            point(R::Center, 0, Some(0));
            handle(R::Start, 0);
            handle(R::End, 0);
            handle(R::Midpoint, 0);
            scalar(0, VALUE_LEAF);
            scalar(1, ANGLE_LEAF);
            scalar(2, ANGLE_LEAF);
        }
        G::ThreePointArc | G::TangentArc => {
            point(R::Center, 0, None);
            handle(R::Start, 0);
            handle(R::End, 0);
            handle(R::Midpoint, 0);
            scalar(0, VALUE_LEAF);
            scalar(1, ANGLE_LEAF);
            scalar(2, ANGLE_LEAF);
        }
        G::CenterAxesEllipse => {
            point(R::Center, 0, Some(0));
            point(R::MajorAxisPoint, 0, Some(1));
            handle(R::MinorAxisPoint, 0);
            scalar(0, PARAMETER_LEAF);
        }
        G::AxisEndpointsEllipse => {
            point(R::Center, 0, None);
            point(R::MajorAxisPoint, 0, Some(0));
            handle(R::End, 0);
            handle(R::MinorAxisPoint, 0);
            scalar(0, PARAMETER_LEAF);
        }
        G::CenterAxesEllipticalArc => {
            point(R::Center, 0, Some(0));
            point(R::MajorAxisPoint, 0, Some(1));
            handle(R::MinorAxisPoint, 0);
            handle(R::Start, 0);
            handle(R::End, 0);
            scalar(0, PARAMETER_LEAF);
            scalar(1, ANGLE_LEAF);
            scalar(2, ANGLE_LEAF);
        }
        G::AxisEndpointsEllipticalArc => {
            point(R::Center, 0, None);
            point(R::MajorAxisPoint, 0, Some(0));
            handle(R::Control, 0);
            handle(R::MinorAxisPoint, 0);
            handle(R::Start, 0);
            handle(R::End, 0);
            scalar(0, PARAMETER_LEAF);
            scalar(1, ANGLE_LEAF);
            scalar(2, ANGLE_LEAF);
        }
        G::QuadraticBezier => {
            point(R::Start, 0, Some(0));
            point(R::Control, 0, Some(1));
            point(R::End, 0, Some(2));
        }
        G::CubicBezier => {
            point(R::Start, 0, Some(0));
            point(R::Control, 0, Some(1));
            point(R::Control, 1, Some(2));
            point(R::End, 0, Some(3));
        }
        G::RationalQuadraticConic => {
            point(R::Start, 0, Some(0));
            handle(R::Control, 0);
            point(R::End, 0, Some(2));
            scalar(0, WEIGHT_LEAF);
        }
        G::Parabola => {
            point(R::Center, 0, Some(0));
            point(R::Control, 0, Some(1));
            scalar(0, PARAMETER_LEAF);
            scalar(1, PARAMETER_LEAF);
        }
        G::Hyperbola => {
            point(R::Center, 0, Some(0));
            point(R::Control, 0, Some(1));
            scalar(0, VALUE_LEAF);
            scalar(1, PARAMETER_LEAF);
            scalar(2, PARAMETER_LEAF);
        }
    }

    let mut specs = specs.into_inner();
    let curve_count = match recipe {
        G::SketchPoint => 0,
        G::TwoPointAlignedRectangle | G::ThreePointCornerRectangle => 4,
        G::CenterRectangle | G::ThreePointCenterRectangle => 5,
        _ => 1,
    };
    for index in 0..curve_count {
        specs.push(PortSpec {
            selector: IntentPortSelector::Node {
                role: R::Curve,
                index,
            },
            kind: IntentPortKind::Curve,
            writable: NO_LEAVES,
            native: Some(IntentNativeReservationKind::Curve),
            alias_input: None,
        });
        specs.push(PortSpec {
            selector: IntentPortSelector::Node {
                role: R::Span,
                index,
            },
            kind: IntentPortKind::CurveSpan,
            writable: NO_LEAVES,
            native: None,
            alias_input: None,
        });
    }
    if matches!(recipe, G::Polyline) {
        specs.push(PortSpec {
            selector: IntentPortSelector::Node {
                role: R::Collection,
                index: 0,
            },
            kind: IntentPortKind::Collection,
            writable: NO_LEAVES,
            native: None,
            alias_input: None,
        });
    }
    specs
}

const fn constraint_contact_count(constraint: ConstraintKind) -> u16 {
    match constraint {
        ConstraintKind::PointOnCurve
        | ConstraintKind::LineCurveTangency
        | ConstraintKind::CurveDirection => 1,
        ConstraintKind::LineCircleTangency
        | ConstraintKind::CircleArcTangency
        | ConstraintKind::CurveCurveContact
        | ConstraintKind::CurveCurveTangency
        | ConstraintKind::EqualCurvature
        | ConstraintKind::EndpointContinuity
        | ConstraintKind::LineLineFillet
        | ConstraintKind::CurveCurveFillet => 2,
        _ => 0,
    }
}

pub(crate) fn literal_matches_leaf(value: &IntentLiteral, field: LeafField) -> bool {
    match field {
        LeafField::X
        | LeafField::Y
        | LeafField::Value
        | LeafField::Angle
        | LeafField::Weight
        | LeafField::Parameter => matches!(value, IntentLiteral::Quantity { .. }),
    }
}

pub(crate) fn next_revision(revision: Revision) -> Option<Revision> {
    revision.raw().checked_add(1).map(Revision::from_raw)
}

pub(crate) fn checked_child_count(count: u16) -> bool {
    usize::from(count) <= MAX_INTENT_NODE_CHILDREN
}

// Keep otherwise schema-owned stable IDs visible in rustdoc and prevent an
// accidental future replacement with caller-authored output manifests.
const _: Option<(ChildId, PortId, ReservationId)> = None;
