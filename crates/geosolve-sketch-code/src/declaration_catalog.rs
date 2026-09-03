// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch_intent::{
    AggregateKind, ComputedFeatureKind, ConstraintKind, DimensionKind, GeometryRecipeKind,
    InputRole, InputSlot, IntentChildSchema, IntentDefinitionFieldDescriptor,
    IntentDraftOutputDescriptor, IntentFieldChoices, IntentFieldDefault, IntentFieldKey, IntentKey,
    IntentLiteral, IntentLiteralSchema, IntentNodeDraft, IntentNodeKind, IntentNodeSchema,
    IntentOperationOutput, IntentPortKind, IntentPortRole, IntentPortSelector,
    IntentProjectionPath, IntentUnit, OperationKind, PatchPortRef,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::FeatureKind;

/// Native declaration identity behind one clean author-facing method.
///
/// This enum is deliberately closed. Transport family strings, semantic
/// paths and result manifests do not enter the public authoring DSL.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "family", content = "kind", rename_all = "snake_case")]
pub enum CodeAuthoringDeclarationKind {
    Geometry(GeometryRecipeKind),
    Constraint(ConstraintKind),
    Dimension(DimensionKind),
    Operation(OperationKind),
    Aggregate(AggregateKind),
    ComputedFeature(ComputedFeatureKind),
}

impl CodeAuthoringDeclarationKind {
    pub(crate) fn intent_kind(self) -> IntentNodeKind {
        match self {
            Self::Geometry(recipe) => IntentNodeKind::Geometry { recipe },
            Self::Constraint(constraint) => IntentNodeKind::Constraint { constraint },
            Self::Dimension(dimension) => IntentNodeKind::Dimension { dimension },
            Self::Operation(operation) => IntentNodeKind::Operation { operation },
            Self::Aggregate(aggregate) => IntentNodeKind::Aggregate { aggregate },
            Self::ComputedFeature(feature) => IntentNodeKind::ComputedFeature { feature },
        }
    }
}

/// Whether an authored method can execute from a standalone managed sketch.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeAuthoringAvailability {
    Public,
    /// The native relation exists, but executing it requires immutable host
    /// snapshot/binding authority which managed source cannot yet provide.
    RequiresHostSnapshot,
}

/// Variable-cardinality structure owned by one authored declaration.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeAuthoringDynamicChildren {
    None,
    PolylineVertices,
    SplineControls,
    FilletCorners,
    PatternInstances,
}

/// How a generated client must interpret the concrete result descriptors.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeAuthoringResultPolicy {
    Static,
    /// One central definition field changes the generated native/logical
    /// output inventory; clients must resolve the concrete descriptor after
    /// that field is known.
    FieldDependent,
    KeyedPolyline,
    KeyedSpline,
    KeyedFilletCorners,
    /// Operation result identity and cardinality are authenticated by the
    /// exact native prepared operation plan, never guessed by TypeScript.
    AuthenticatedOperationPlan,
}

/// Stable inventory entry used to generate clean TypeScript namespaces.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct CodeAuthoringFamilyDescriptor {
    pub namespace: &'static str,
    pub method: &'static str,
    pub declaration: CodeAuthoringDeclarationKind,
    pub availability: CodeAuthoringAvailability,
    pub dynamic_children: CodeAuthoringDynamicChildren,
    pub result_policy: CodeAuthoringResultPolicy,
}

macro_rules! authoring_family {
    ($namespace:literal, $method:literal, $declaration:expr) => {
        CodeAuthoringFamilyDescriptor {
            namespace: $namespace,
            method: $method,
            declaration: $declaration,
            availability: CodeAuthoringAvailability::Public,
            dynamic_children: CodeAuthoringDynamicChildren::None,
            result_policy: CodeAuthoringResultPolicy::Static,
        }
    };
    (
        $namespace:literal,
        $method:literal,
        $declaration:expr,
        $availability:expr,
        $children:expr,
        $results:expr
    ) => {
        CodeAuthoringFamilyDescriptor {
            namespace: $namespace,
            method: $method,
            declaration: $declaration,
            availability: $availability,
            dynamic_children: $children,
            result_policy: $results,
        }
    };
}

/// Complete clean-break authoring inventory.
///
/// The 83 public entries comprise 27 geometry recipes, 33 standalone
/// constraints, eight dimensions, twelve operations, two aggregates and
/// computed `FilletSet`. The two remaining native external constraints stay
/// present but explicitly gated, for 85 entries in total.
#[allow(
    clippy::too_many_lines,
    reason = "one closed table keeps every public authoring name reviewable beside its Rust identity"
)]
pub const CODE_AUTHORING_FAMILIES: [CodeAuthoringFamilyDescriptor; 85] = [
    authoring_family!(
        "geometry",
        "sketchPoint",
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::SketchPoint)
    ),
    authoring_family!(
        "geometry",
        "segment",
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::Segment)
    ),
    authoring_family!(
        "geometry",
        "polyline",
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::Polyline),
        CodeAuthoringAvailability::Public,
        CodeAuthoringDynamicChildren::PolylineVertices,
        CodeAuthoringResultPolicy::KeyedPolyline
    ),
    authoring_family!(
        "geometry",
        "midpointLine",
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::MidpointLine)
    ),
    authoring_family!(
        "geometry",
        "twoPointAlignedRectangle",
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::TwoPointAlignedRectangle),
        CodeAuthoringAvailability::Public,
        CodeAuthoringDynamicChildren::None,
        CodeAuthoringResultPolicy::FieldDependent
    ),
    authoring_family!(
        "geometry",
        "threePointCornerRectangle",
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::ThreePointCornerRectangle),
        CodeAuthoringAvailability::Public,
        CodeAuthoringDynamicChildren::None,
        CodeAuthoringResultPolicy::FieldDependent
    ),
    authoring_family!(
        "geometry",
        "centerRectangle",
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::CenterRectangle),
        CodeAuthoringAvailability::Public,
        CodeAuthoringDynamicChildren::None,
        CodeAuthoringResultPolicy::FieldDependent
    ),
    authoring_family!(
        "geometry",
        "threePointCenterRectangle",
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::ThreePointCenterRectangle),
        CodeAuthoringAvailability::Public,
        CodeAuthoringDynamicChildren::None,
        CodeAuthoringResultPolicy::FieldDependent
    ),
    authoring_family!(
        "geometry",
        "centerRadiusCircle",
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::CenterRadiusCircle)
    ),
    authoring_family!(
        "geometry",
        "twoPointDiameterCircle",
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::TwoPointDiameterCircle)
    ),
    authoring_family!(
        "geometry",
        "threePointCircle",
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::ThreePointCircle)
    ),
    authoring_family!(
        "geometry",
        "centerArc",
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::CenterArc)
    ),
    authoring_family!(
        "geometry",
        "threePointArc",
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::ThreePointArc)
    ),
    authoring_family!(
        "geometry",
        "tangentArc",
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::TangentArc)
    ),
    authoring_family!(
        "geometry",
        "centerAxesEllipse",
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::CenterAxesEllipse)
    ),
    authoring_family!(
        "geometry",
        "axisEndpointsEllipse",
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::AxisEndpointsEllipse)
    ),
    authoring_family!(
        "geometry",
        "centerAxesEllipticalArc",
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::CenterAxesEllipticalArc)
    ),
    authoring_family!(
        "geometry",
        "axisEndpointsEllipticalArc",
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::AxisEndpointsEllipticalArc)
    ),
    authoring_family!(
        "geometry",
        "quadraticBezier",
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::QuadraticBezier)
    ),
    authoring_family!(
        "geometry",
        "cubicBezier",
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::CubicBezier)
    ),
    authoring_family!(
        "geometry",
        "rationalQuadraticConic",
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::RationalQuadraticConic)
    ),
    authoring_family!(
        "geometry",
        "parabola",
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::Parabola)
    ),
    authoring_family!(
        "geometry",
        "hyperbola",
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::Hyperbola)
    ),
    authoring_family!(
        "geometry",
        "openControlBSpline",
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::OpenControlBSpline),
        CodeAuthoringAvailability::Public,
        CodeAuthoringDynamicChildren::SplineControls,
        CodeAuthoringResultPolicy::KeyedSpline
    ),
    authoring_family!(
        "geometry",
        "periodicControlBSpline",
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::PeriodicControlBSpline),
        CodeAuthoringAvailability::Public,
        CodeAuthoringDynamicChildren::SplineControls,
        CodeAuthoringResultPolicy::KeyedSpline
    ),
    authoring_family!(
        "geometry",
        "openControlNurbs",
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::OpenControlNurbs),
        CodeAuthoringAvailability::Public,
        CodeAuthoringDynamicChildren::SplineControls,
        CodeAuthoringResultPolicy::KeyedSpline
    ),
    authoring_family!(
        "geometry",
        "periodicControlNurbs",
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::PeriodicControlNurbs),
        CodeAuthoringAvailability::Public,
        CodeAuthoringDynamicChildren::SplineControls,
        CodeAuthoringResultPolicy::KeyedSpline
    ),
    authoring_family!(
        "constraint",
        "fixedPoint",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::FixedPoint)
    ),
    authoring_family!(
        "constraint",
        "fixedCoordinate",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::FixedCoordinate)
    ),
    authoring_family!(
        "constraint",
        "coincidentWithOrigin",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::CoincidentWithOrigin)
    ),
    authoring_family!(
        "constraint",
        "pointOnDatumAxis",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::PointOnDatumAxis)
    ),
    authoring_family!(
        "constraint",
        "coincident",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::Coincident)
    ),
    authoring_family!(
        "constraint",
        "externalPointCoincident",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::ExternalPointCoincident),
        CodeAuthoringAvailability::RequiresHostSnapshot,
        CodeAuthoringDynamicChildren::None,
        CodeAuthoringResultPolicy::Static
    ),
    authoring_family!(
        "constraint",
        "horizontal",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::Horizontal)
    ),
    authoring_family!(
        "constraint",
        "vertical",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::Vertical)
    ),
    authoring_family!(
        "constraint",
        "horizontalPoints",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::HorizontalPoints)
    ),
    authoring_family!(
        "constraint",
        "verticalPoints",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::VerticalPoints)
    ),
    authoring_family!(
        "constraint",
        "horizontalPointToMidpoint",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::HorizontalPointToMidpoint)
    ),
    authoring_family!(
        "constraint",
        "verticalPointToMidpoint",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::VerticalPointToMidpoint)
    ),
    authoring_family!(
        "constraint",
        "pointOnCurve",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::PointOnCurve)
    ),
    authoring_family!(
        "constraint",
        "parallel",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::Parallel)
    ),
    authoring_family!(
        "constraint",
        "perpendicular",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::Perpendicular)
    ),
    authoring_family!(
        "constraint",
        "externalLineCollinear",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::ExternalLineCollinear),
        CodeAuthoringAvailability::RequiresHostSnapshot,
        CodeAuthoringDynamicChildren::None,
        CodeAuthoringResultPolicy::Static
    ),
    authoring_family!(
        "constraint",
        "collinearWithDatumAxis",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::CollinearWithDatumAxis)
    ),
    authoring_family!(
        "constraint",
        "concentric",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::Concentric)
    ),
    authoring_family!(
        "constraint",
        "collinear",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::Collinear)
    ),
    authoring_family!(
        "constraint",
        "equalLength",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::EqualLength)
    ),
    authoring_family!(
        "constraint",
        "equalRadius",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::EqualRadius)
    ),
    authoring_family!(
        "constraint",
        "midpoint",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::Midpoint)
    ),
    authoring_family!(
        "constraint",
        "symmetricAboutLine",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::SymmetricAboutLine)
    ),
    authoring_family!(
        "constraint",
        "symmetricAboutDatumAxis",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::SymmetricAboutDatumAxis)
    ),
    authoring_family!(
        "constraint",
        "lineCircleTangency",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::LineCircleTangency)
    ),
    authoring_family!(
        "constraint",
        "circleCircleTangency",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::CircleCircleTangency)
    ),
    authoring_family!(
        "constraint",
        "circleArcTangency",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::CircleArcTangency)
    ),
    authoring_family!(
        "constraint",
        "lineCurveTangency",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::LineCurveTangency)
    ),
    authoring_family!(
        "constraint",
        "curveCurveContact",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::CurveCurveContact)
    ),
    authoring_family!(
        "constraint",
        "curveCurveTangency",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::CurveCurveTangency)
    ),
    authoring_family!(
        "constraint",
        "curveDirection",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::CurveDirection)
    ),
    authoring_family!(
        "constraint",
        "equalCurvature",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::EqualCurvature)
    ),
    authoring_family!(
        "constraint",
        "endpointContinuity",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::EndpointContinuity)
    ),
    authoring_family!(
        "constraint",
        "lineLineFillet",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::LineLineFillet)
    ),
    authoring_family!(
        "constraint",
        "curveCurveFillet",
        CodeAuthoringDeclarationKind::Constraint(ConstraintKind::CurveCurveFillet)
    ),
    authoring_family!(
        "dimension",
        "pointDistance",
        CodeAuthoringDeclarationKind::Dimension(DimensionKind::PointDistance)
    ),
    authoring_family!(
        "dimension",
        "curveLength",
        CodeAuthoringDeclarationKind::Dimension(DimensionKind::CurveLength)
    ),
    authoring_family!(
        "dimension",
        "radius",
        CodeAuthoringDeclarationKind::Dimension(DimensionKind::Radius)
    ),
    authoring_family!(
        "dimension",
        "diameter",
        CodeAuthoringDeclarationKind::Dimension(DimensionKind::Diameter)
    ),
    authoring_family!(
        "dimension",
        "orientedAngle",
        CodeAuthoringDeclarationKind::Dimension(DimensionKind::OrientedAngle)
    ),
    authoring_family!(
        "dimension",
        "supportingLineOffset",
        CodeAuthoringDeclarationKind::Dimension(DimensionKind::SupportingLineOffset)
    ),
    authoring_family!(
        "dimension",
        "exactTranslatedSegmentOffset",
        CodeAuthoringDeclarationKind::Dimension(DimensionKind::ExactTranslatedSegmentOffset)
    ),
    authoring_family!(
        "dimension",
        "profileOffset",
        CodeAuthoringDeclarationKind::Dimension(DimensionKind::ProfileOffset)
    ),
    authoring_family!(
        "operation",
        "split",
        CodeAuthoringDeclarationKind::Operation(OperationKind::Split),
        CodeAuthoringAvailability::Public,
        CodeAuthoringDynamicChildren::None,
        CodeAuthoringResultPolicy::AuthenticatedOperationPlan
    ),
    authoring_family!(
        "operation",
        "break",
        CodeAuthoringDeclarationKind::Operation(OperationKind::Break),
        CodeAuthoringAvailability::Public,
        CodeAuthoringDynamicChildren::None,
        CodeAuthoringResultPolicy::AuthenticatedOperationPlan
    ),
    authoring_family!(
        "operation",
        "trim",
        CodeAuthoringDeclarationKind::Operation(OperationKind::Trim),
        CodeAuthoringAvailability::Public,
        CodeAuthoringDynamicChildren::None,
        CodeAuthoringResultPolicy::AuthenticatedOperationPlan
    ),
    authoring_family!(
        "operation",
        "extend",
        CodeAuthoringDeclarationKind::Operation(OperationKind::Extend),
        CodeAuthoringAvailability::Public,
        CodeAuthoringDynamicChildren::None,
        CodeAuthoringResultPolicy::AuthenticatedOperationPlan
    ),
    authoring_family!(
        "operation",
        "mirror",
        CodeAuthoringDeclarationKind::Operation(OperationKind::Mirror),
        CodeAuthoringAvailability::Public,
        CodeAuthoringDynamicChildren::None,
        CodeAuthoringResultPolicy::AuthenticatedOperationPlan
    ),
    authoring_family!(
        "operation",
        "chamfer",
        CodeAuthoringDeclarationKind::Operation(OperationKind::Chamfer),
        CodeAuthoringAvailability::Public,
        CodeAuthoringDynamicChildren::None,
        CodeAuthoringResultPolicy::AuthenticatedOperationPlan
    ),
    authoring_family!(
        "operation",
        "associativeFillet",
        CodeAuthoringDeclarationKind::Operation(OperationKind::AssociativeFillet),
        CodeAuthoringAvailability::Public,
        CodeAuthoringDynamicChildren::None,
        CodeAuthoringResultPolicy::AuthenticatedOperationPlan
    ),
    authoring_family!(
        "operation",
        "rectangle",
        CodeAuthoringDeclarationKind::Operation(OperationKind::Rectangle),
        CodeAuthoringAvailability::Public,
        CodeAuthoringDynamicChildren::None,
        CodeAuthoringResultPolicy::AuthenticatedOperationPlan
    ),
    authoring_family!(
        "operation",
        "regularPolygon",
        CodeAuthoringDeclarationKind::Operation(OperationKind::RegularPolygon),
        CodeAuthoringAvailability::Public,
        CodeAuthoringDynamicChildren::None,
        CodeAuthoringResultPolicy::AuthenticatedOperationPlan
    ),
    authoring_family!(
        "operation",
        "slot",
        CodeAuthoringDeclarationKind::Operation(OperationKind::Slot),
        CodeAuthoringAvailability::Public,
        CodeAuthoringDynamicChildren::None,
        CodeAuthoringResultPolicy::AuthenticatedOperationPlan
    ),
    authoring_family!(
        "operation",
        "linearPattern",
        CodeAuthoringDeclarationKind::Operation(OperationKind::LinearPattern),
        CodeAuthoringAvailability::Public,
        CodeAuthoringDynamicChildren::PatternInstances,
        CodeAuthoringResultPolicy::AuthenticatedOperationPlan
    ),
    authoring_family!(
        "operation",
        "profileOffset",
        CodeAuthoringDeclarationKind::Operation(OperationKind::ProfileOffset),
        CodeAuthoringAvailability::Public,
        CodeAuthoringDynamicChildren::None,
        CodeAuthoringResultPolicy::AuthenticatedOperationPlan
    ),
    authoring_family!(
        "aggregate",
        "openChain",
        CodeAuthoringDeclarationKind::Aggregate(AggregateKind::OpenChain)
    ),
    authoring_family!(
        "aggregate",
        "closedProfile",
        CodeAuthoringDeclarationKind::Aggregate(AggregateKind::ClosedProfile)
    ),
    authoring_family!(
        "computed",
        "filletSet",
        CodeAuthoringDeclarationKind::ComputedFeature(ComputedFeatureKind::FilletSet),
        CodeAuthoringAvailability::Public,
        CodeAuthoringDynamicChildren::FilletCorners,
        CodeAuthoringResultPolicy::KeyedFilletCorners
    ),
];

/// Semantic value accepted by one named authoring argument.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "value", content = "detail", rename_all = "snake_case")]
pub enum CodeAuthoringArgumentKind {
    /// A Cartesian point literal or a typed stable Point reference.
    Point,
    Reference(IntentPortKind),
    ReferenceChoice(Vec<IntentPortKind>),
    Collection(CodeAuthoringCollectionMember),
}

/// Member shape of a variable-cardinality authoring argument.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "member", content = "kind", rename_all = "snake_case")]
pub enum CodeAuthoringCollectionMember {
    Point,
    /// One keyed control owns a point plus its dimensionless weight.
    SplineControl,
    /// One Fillet corner owns two span parents and complete branch/contact
    /// metadata. Its exact fields are generated for each concrete child count.
    FilletCorner,
    /// One associative-Fillet parent owns a span plus its complete explicit
    /// contact, trim, normal-side, and periodic-anchor branch state.
    FilletParent,
    Reference(IntentPortKind),
    ReferenceChoice(Vec<IntentPortKind>),
}

/// Internal mapping from one clean named argument to central Intent slots.
///
/// This metadata is generator input, not a public raw-path escape hatch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CodeAuthoringInputBinding {
    Slot { slot: InputSlot },
    Role { role: InputRole },
    Roles { roles: Vec<InputRole> },
    DynamicChildren { schema: IntentChildSchema },
}

/// One clean, semantically named authoring argument.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CodeAuthoringInputDescriptor {
    pub name: String,
    pub kind: CodeAuthoringArgumentKind,
    pub minimum: u16,
    pub maximum: u16,
    #[serde(skip_serializing)]
    pub(crate) binding: CodeAuthoringInputBinding,
}

/// Writable authored value which is not a dependency binding or definition
/// field. This keeps scalar authoring inputs such as a circle radius or a
/// dimension target distinct from native construction handles.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CodeAuthoringValueDescriptor {
    pub path: IntentProjectionPath,
    pub literal: IntentLiteralSchema,
    pub default: IntentFieldDefault,
    pub choices: IntentFieldChoices,
}

/// Concrete dynamic-child bounds for one resolved method invocation shape.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CodeAuthoringDynamicChildrenDescriptor {
    pub kind: CodeAuthoringDynamicChildren,
    pub minimum: u16,
    pub maximum: u16,
    pub count: u16,
}

/// Fully resolved, equation-free authoring descriptor.
///
/// Definition fields and outputs are copied from the central Intent
/// descriptors for the requested child count. The code adapter owns only the
/// ergonomic name/binding layer around that authority.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CodeAuthoringDeclarationDescriptor {
    pub namespace: &'static str,
    pub method: &'static str,
    pub declaration: CodeAuthoringDeclarationKind,
    pub availability: CodeAuthoringAvailability,
    pub dynamic_children: CodeAuthoringDynamicChildrenDescriptor,
    pub inputs: Vec<CodeAuthoringInputDescriptor>,
    pub fields: Vec<IntentDefinitionFieldDescriptor>,
    pub values: Vec<CodeAuthoringValueDescriptor>,
    pub outputs: Vec<IntentDraftOutputDescriptor>,
    pub result_policy: CodeAuthoringResultPolicy,
}

/// Failure to resolve one source-controlled authoring catalog entry.
#[derive(Clone, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum CodeAuthoringCatalogError {
    #[error("unknown authoring method `{namespace}.{method}`")]
    UnknownMethod { namespace: String, method: String },
    #[error(
        "`{namespace}.{method}` requires between {minimum} and {maximum} dynamic children, got {actual}"
    )]
    InvalidDynamicChildren {
        namespace: &'static str,
        method: &'static str,
        minimum: u16,
        maximum: u16,
        actual: u16,
    },
    #[error("the Rust-owned `{namespace}.{method}` catalog is inconsistent: {reason}")]
    InconsistentCatalog {
        namespace: &'static str,
        method: &'static str,
        reason: String,
    },
}

/// Looks up one clean author-facing method.
#[must_use]
pub fn code_authoring_family(
    namespace: &str,
    method: &str,
) -> Option<&'static CodeAuthoringFamilyDescriptor> {
    CODE_AUTHORING_FAMILIES
        .iter()
        .find(|entry| entry.namespace == namespace && entry.method == method)
}

/// Iterates only the standalone-authoritative public method inventory.
pub fn public_code_authoring_families()
-> impl Iterator<Item = &'static CodeAuthoringFamilyDescriptor> {
    CODE_AUTHORING_FAMILIES
        .iter()
        .filter(|entry| entry.availability == CodeAuthoringAvailability::Public)
}

/// Resolves one clean method against the central Intent descriptor at an
/// exact dynamic-child cardinality.
///
/// # Errors
///
/// Returns a typed error for an unknown method, invalid child cardinality or
/// any drift between this ergonomic catalog and the central Intent schema.
pub fn resolve_code_authoring_declaration(
    namespace: &str,
    method: &str,
    dynamic_children: u16,
) -> Result<CodeAuthoringDeclarationDescriptor, CodeAuthoringCatalogError> {
    let family = code_authoring_family(namespace, method).ok_or_else(|| {
        CodeAuthoringCatalogError::UnknownMethod {
            namespace: namespace.to_owned(),
            method: method.to_owned(),
        }
    })?;
    resolve_code_authoring_family(family, dynamic_children)
}

fn resolve_code_authoring_family(
    family: &'static CodeAuthoringFamilyDescriptor,
    dynamic_children: u16,
) -> Result<CodeAuthoringDeclarationDescriptor, CodeAuthoringCatalogError> {
    let kind = family.declaration.intent_kind();
    let schema = kind.schema(dynamic_children);
    let (minimum, maximum) = (schema.minimum_children, schema.maximum_children);
    let child_count_valid = if family.dynamic_children == CodeAuthoringDynamicChildren::None {
        dynamic_children == 0 && minimum == 0 && maximum == 0
    } else {
        (minimum..=maximum).contains(&dynamic_children)
    };
    if !child_count_valid {
        return Err(CodeAuthoringCatalogError::InvalidDynamicChildren {
            namespace: family.namespace,
            method: family.method,
            minimum,
            maximum,
            actual: dynamic_children,
        });
    }
    if dynamic_children_kind(&kind) != family.dynamic_children {
        return Err(inconsistent(
            family,
            "dynamic-child policy does not match the central declaration kind",
        ));
    }

    let inputs = authoring_inputs(family, &schema)?;
    validate_input_coverage(family, &schema, &inputs)?;
    let fields = kind.field_descriptors(dynamic_children);
    let values = authoring_values(family.declaration);
    let outputs = representative_outputs(family, &kind, dynamic_children, &fields, &schema)?;

    Ok(CodeAuthoringDeclarationDescriptor {
        namespace: family.namespace,
        method: family.method,
        declaration: family.declaration,
        availability: family.availability,
        dynamic_children: CodeAuthoringDynamicChildrenDescriptor {
            kind: family.dynamic_children,
            minimum,
            maximum,
            count: dynamic_children,
        },
        inputs,
        fields,
        values,
        outputs,
        result_policy: family.result_policy,
    })
}

const fn dynamic_children_kind(kind: &IntentNodeKind) -> CodeAuthoringDynamicChildren {
    match kind {
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Polyline,
        } => CodeAuthoringDynamicChildren::PolylineVertices,
        IntentNodeKind::Geometry {
            recipe:
                GeometryRecipeKind::OpenControlBSpline
                | GeometryRecipeKind::PeriodicControlBSpline
                | GeometryRecipeKind::OpenControlNurbs
                | GeometryRecipeKind::PeriodicControlNurbs,
        } => CodeAuthoringDynamicChildren::SplineControls,
        IntentNodeKind::ComputedFeature {
            feature: ComputedFeatureKind::FilletSet,
        } => CodeAuthoringDynamicChildren::FilletCorners,
        IntentNodeKind::Operation {
            operation: OperationKind::LinearPattern,
        } => CodeAuthoringDynamicChildren::PatternInstances,
        IntentNodeKind::Geometry { .. }
        | IntentNodeKind::Constraint { .. }
        | IntentNodeKind::Dimension { .. }
        | IntentNodeKind::Operation { .. }
        | IntentNodeKind::Aggregate { .. }
        | IntentNodeKind::Parameter { .. }
        | IntentNodeKind::External { .. }
        | IntentNodeKind::Bootstrap { .. }
        | IntentNodeKind::Annotation
        | IntentNodeKind::Identity { .. } => CodeAuthoringDynamicChildren::None,
    }
}

fn inconsistent(
    family: &CodeAuthoringFamilyDescriptor,
    reason: impl Into<String>,
) -> CodeAuthoringCatalogError {
    CodeAuthoringCatalogError::InconsistentCatalog {
        namespace: family.namespace,
        method: family.method,
        reason: reason.into(),
    }
}

fn authoring_inputs(
    family: &CodeAuthoringFamilyDescriptor,
    schema: &IntentNodeSchema,
) -> Result<Vec<CodeAuthoringInputDescriptor>, CodeAuthoringCatalogError> {
    let inputs = match family.declaration {
        CodeAuthoringDeclarationKind::Geometry(recipe) => geometry_authoring_inputs(recipe, schema),
        CodeAuthoringDeclarationKind::Constraint(constraint) => {
            constraint_authoring_inputs(constraint)
        }
        CodeAuthoringDeclarationKind::Dimension(dimension) => dimension_authoring_inputs(dimension),
        CodeAuthoringDeclarationKind::Operation(operation) => operation_authoring_inputs(operation),
        CodeAuthoringDeclarationKind::Aggregate(_) => vec![role_collection(
            "spans",
            InputRole::Span,
            IntentPortKind::CurveSpan,
            1,
            role_maximum(schema, InputRole::Span),
        )],
        CodeAuthoringDeclarationKind::ComputedFeature(ComputedFeatureKind::FilletSet) => {
            vec![dynamic_collection(
                "corners",
                CodeAuthoringCollectionMember::FilletCorner,
                IntentChildSchema::FilletCorner,
                schema.minimum_children,
                schema.maximum_children,
            )]
        }
    };
    if inputs.iter().any(|input| input.maximum < input.minimum) {
        return Err(inconsistent(family, "an input has inverted cardinality"));
    }
    Ok(inputs)
}

fn geometry_authoring_inputs(
    recipe: GeometryRecipeKind,
    schema: &IntentNodeSchema,
) -> Vec<CodeAuthoringInputDescriptor> {
    use GeometryRecipeKind as G;

    match recipe {
        G::Polyline => vec![dynamic_collection(
            "vertices",
            CodeAuthoringCollectionMember::Point,
            IntentChildSchema::PolylineVertex,
            schema.minimum_children,
            schema.maximum_children,
        )],
        G::OpenControlBSpline
        | G::PeriodicControlBSpline
        | G::OpenControlNurbs
        | G::PeriodicControlNurbs => vec![dynamic_collection(
            "controls",
            CodeAuthoringCollectionMember::SplineControl,
            IntentChildSchema::SplineControl,
            schema.minimum_children,
            schema.maximum_children,
        )],
        _ => {
            // Center-radius authoring is intentionally scalar-first. The
            // native recipe retains a derived radius-point handle for direct
            // manipulation, but that handle is an output rather than a
            // required source operand.
            let point_count = if recipe == G::CenterRadiusCircle {
                1
            } else {
                role_maximum(schema, InputRole::Point)
            };
            let names = geometry_point_names(recipe);
            let mut inputs = (0..point_count)
                .map(|index| {
                    point_slot(
                        names.get(usize::from(index)).copied().unwrap_or("point"),
                        index,
                    )
                })
                .collect::<Vec<_>>();
            if recipe == G::TangentArc {
                inputs.push(reference_slot(
                    "source",
                    InputRole::Span,
                    0,
                    IntentPortKind::CurveSpan,
                ));
            }
            inputs
        }
    }
}

const fn geometry_point_names(recipe: GeometryRecipeKind) -> &'static [&'static str] {
    use GeometryRecipeKind as G;
    match recipe {
        G::SketchPoint => &["point"],
        G::Segment | G::TwoPointDiameterCircle | G::TangentArc | G::RationalQuadraticConic => {
            &["start", "end"]
        }
        G::MidpointLine => &["midpoint", "end"],
        G::TwoPointAlignedRectangle => &["firstCorner", "oppositeCorner"],
        G::ThreePointCornerRectangle => &["firstCorner", "secondCorner", "thirdCorner"],
        G::CenterRectangle | G::ThreePointCenterRectangle => &["center", "corner"],
        G::CenterRadiusCircle => &["center", "radiusPoint"],
        G::ThreePointCircle | G::ThreePointArc => &["first", "second", "third"],
        G::CenterArc => &["center", "start", "end"],
        G::CenterAxesEllipse | G::CenterAxesEllipticalArc => {
            &["center", "majorAxisPoint", "minorAxisPoint", "start", "end"]
        }
        G::AxisEndpointsEllipse | G::AxisEndpointsEllipticalArc => &[
            "majorAxisStart",
            "majorAxisEnd",
            "minorAxisPoint",
            "start",
            "end",
        ],
        G::QuadraticBezier => &["start", "control", "end"],
        G::CubicBezier => &["start", "firstControl", "secondControl", "end"],
        G::Parabola => &["vertex", "focus"],
        G::Hyperbola => &["center", "transverseAxisPoint"],
        G::Polyline
        | G::OpenControlBSpline
        | G::PeriodicControlBSpline
        | G::OpenControlNurbs
        | G::PeriodicControlNurbs => &[],
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "the clean constraint signatures remain explicit and reviewable"
)]
fn constraint_authoring_inputs(kind: ConstraintKind) -> Vec<CodeAuthoringInputDescriptor> {
    use ConstraintKind as C;
    match kind {
        C::FixedPoint | C::FixedCoordinate | C::CoincidentWithOrigin | C::PointOnDatumAxis => {
            vec![reference_slot(
                "point",
                InputRole::Point,
                0,
                IntentPortKind::Point,
            )]
        }
        C::Coincident | C::HorizontalPoints | C::VerticalPoints => {
            reference_pair(InputRole::Point, IntentPortKind::Point)
        }
        C::ExternalPointCoincident => vec![
            reference_slot("point", InputRole::Point, 0, IntentPortKind::Point),
            reference_slot(
                "external",
                InputRole::External,
                0,
                IntentPortKind::ExternalBinding,
            ),
        ],
        C::Horizontal | C::Vertical | C::CollinearWithDatumAxis => vec![reference_slot(
            "span",
            InputRole::Span,
            0,
            IntentPortKind::CurveSpan,
        )],
        C::HorizontalPointToMidpoint | C::VerticalPointToMidpoint | C::Midpoint => vec![
            reference_slot("point", InputRole::Point, 0, IntentPortKind::Point),
            reference_slot("line", InputRole::Span, 0, IntentPortKind::CurveSpan),
        ],
        C::PointOnCurve => vec![
            reference_slot("point", InputRole::Point, 0, IntentPortKind::Point),
            reference_slot("curve", InputRole::Span, 0, IntentPortKind::CurveSpan),
        ],
        C::Parallel | C::Perpendicular | C::Collinear | C::EqualLength => {
            reference_pair(InputRole::Span, IntentPortKind::CurveSpan)
        }
        C::ExternalLineCollinear => vec![
            reference_slot("line", InputRole::Span, 0, IntentPortKind::CurveSpan),
            reference_slot(
                "external",
                InputRole::External,
                0,
                IntentPortKind::ExternalBinding,
            ),
        ],
        C::Concentric | C::EqualRadius | C::CircleCircleTangency => {
            reference_pair(InputRole::Curve, IntentPortKind::Curve)
        }
        C::SymmetricAboutLine => vec![
            reference_slot("first", InputRole::Point, 0, IntentPortKind::Point),
            reference_slot("second", InputRole::Point, 1, IntentPortKind::Point),
            reference_slot("axis", InputRole::Span, 0, IntentPortKind::CurveSpan),
        ],
        C::SymmetricAboutDatumAxis => reference_pair(InputRole::Point, IntentPortKind::Point),
        C::LineCircleTangency => vec![
            reference_slot("line", InputRole::Span, 0, IntentPortKind::CurveSpan),
            reference_slot("circle", InputRole::Curve, 0, IntentPortKind::Curve),
        ],
        C::CircleArcTangency => vec![
            reference_slot("circle", InputRole::Curve, 0, IntentPortKind::Curve),
            reference_slot("arc", InputRole::Curve, 1, IntentPortKind::Curve),
        ],
        C::LineCurveTangency => vec![
            reference_slot("line", InputRole::Span, 0, IntentPortKind::CurveSpan),
            reference_slot("curve", InputRole::Span, 1, IntentPortKind::CurveSpan),
        ],
        C::CurveCurveContact
        | C::CurveCurveTangency
        | C::CurveDirection
        | C::EqualCurvature
        | C::EndpointContinuity => reference_pair(InputRole::Span, IntentPortKind::CurveSpan),
        C::LineLineFillet | C::CurveCurveFillet => vec![
            reference_slot("fillet", InputRole::Curve, 0, IntentPortKind::Curve),
            reference_slot("first", InputRole::Span, 0, IntentPortKind::CurveSpan),
            reference_slot("second", InputRole::Span, 1, IntentPortKind::CurveSpan),
        ],
    }
}

fn dimension_authoring_inputs(kind: DimensionKind) -> Vec<CodeAuthoringInputDescriptor> {
    use DimensionKind as D;
    match kind {
        D::PointDistance => reference_pair(InputRole::Point, IntentPortKind::Point),
        D::CurveLength => vec![reference_slot(
            "curve",
            InputRole::Span,
            0,
            IntentPortKind::CurveSpan,
        )],
        D::Radius | D::Diameter => vec![reference_slot(
            "curve",
            InputRole::Curve,
            0,
            IntentPortKind::Curve,
        )],
        D::OrientedAngle | D::SupportingLineOffset | D::ExactTranslatedSegmentOffset => {
            reference_pair(InputRole::Span, IntentPortKind::CurveSpan)
        }
        D::ProfileOffset => vec![
            CodeAuthoringInputDescriptor {
                name: "source".into(),
                kind: CodeAuthoringArgumentKind::ReferenceChoice(vec![
                    IntentPortKind::Profile,
                    IntentPortKind::Chain,
                ]),
                minimum: 1,
                maximum: 1,
                binding: CodeAuthoringInputBinding::Roles {
                    roles: vec![InputRole::Profile, InputRole::Chain],
                },
            },
            optional_reference_slot("target", InputRole::Span, 0, IntentPortKind::CurveSpan),
        ],
    }
}

fn operation_authoring_inputs(kind: OperationKind) -> Vec<CodeAuthoringInputDescriptor> {
    use OperationKind as O;
    match kind {
        O::Split | O::Break | O::Trim => vec![reference_slot(
            "source",
            InputRole::Span,
            0,
            IntentPortKind::CurveSpan,
        )],
        O::Extend => vec![
            reference_slot("source", InputRole::Span, 0, IntentPortKind::CurveSpan),
            reference_slot("target", InputRole::Span, 1, IntentPortKind::CurveSpan),
        ],
        O::Mirror => vec![
            reference_slot("source", InputRole::Curve, 0, IntentPortKind::Curve),
            reference_slot("axis", InputRole::Span, 0, IntentPortKind::CurveSpan),
        ],
        O::Chamfer => reference_pair(InputRole::Span, IntentPortKind::CurveSpan),
        O::AssociativeFillet => vec![CodeAuthoringInputDescriptor {
            name: "parents".into(),
            kind: CodeAuthoringArgumentKind::Collection(
                CodeAuthoringCollectionMember::FilletParent,
            ),
            minimum: 2,
            maximum: 2,
            binding: CodeAuthoringInputBinding::Role {
                role: InputRole::Span,
            },
        }],
        O::Rectangle | O::RegularPolygon | O::Slot => Vec::new(),
        O::LinearPattern => vec![role_collection(
            "sources",
            InputRole::Curve,
            IntentPortKind::Curve,
            1,
            4_096,
        )],
        O::ProfileOffset => vec![CodeAuthoringInputDescriptor {
            name: "sources".into(),
            kind: CodeAuthoringArgumentKind::Collection(
                CodeAuthoringCollectionMember::ReferenceChoice(vec![
                    IntentPortKind::Profile,
                    IntentPortKind::Chain,
                ]),
            ),
            minimum: 1,
            maximum: 4_096,
            binding: CodeAuthoringInputBinding::Roles {
                roles: vec![InputRole::Profile, InputRole::Chain],
            },
        }],
    }
}

fn authoring_values(
    declaration: CodeAuthoringDeclarationKind,
) -> Vec<CodeAuthoringValueDescriptor> {
    let values: &[(&str, IntentUnit)] = match declaration {
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::CenterRadiusCircle) => {
            &[("radius", IntentUnit::Length)]
        }
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::RationalQuadraticConic) => {
            &[("middleWeight", IntentUnit::Dimensionless)]
        }
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::Parabola) => &[
            ("trimStart", IntentUnit::Dimensionless),
            ("trimEnd", IntentUnit::Dimensionless),
        ],
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::Hyperbola) => &[
            ("semiConjugate", IntentUnit::Length),
            ("trimStart", IntentUnit::Dimensionless),
            ("trimEnd", IntentUnit::Dimensionless),
        ],
        CodeAuthoringDeclarationKind::Dimension(DimensionKind::OrientedAngle) => {
            &[("value", IntentUnit::Angle)]
        }
        CodeAuthoringDeclarationKind::Dimension(_) => &[("value", IntentUnit::Length)],
        CodeAuthoringDeclarationKind::Geometry(_)
        | CodeAuthoringDeclarationKind::Constraint(_)
        | CodeAuthoringDeclarationKind::Operation(_)
        | CodeAuthoringDeclarationKind::Aggregate(_)
        | CodeAuthoringDeclarationKind::ComputedFeature(_) => return Vec::new(),
    };
    values
        .iter()
        .map(|(name, unit)| CodeAuthoringValueDescriptor {
            path: IntentProjectionPath::field(
                IntentKey::new(*name).expect("source-controlled value path is valid"),
            ),
            literal: IntentLiteralSchema::Quantity(*unit),
            default: IntentFieldDefault::Required,
            choices: IntentFieldChoices::NotApplicable,
        })
        .collect()
}

fn point_slot(name: &str, index: u16) -> CodeAuthoringInputDescriptor {
    CodeAuthoringInputDescriptor {
        name: name.into(),
        kind: CodeAuthoringArgumentKind::Point,
        minimum: 1,
        maximum: 1,
        binding: CodeAuthoringInputBinding::Slot {
            slot: InputSlot::new(InputRole::Point, index),
        },
    }
}

fn reference_slot(
    name: &str,
    role: InputRole,
    index: u16,
    kind: IntentPortKind,
) -> CodeAuthoringInputDescriptor {
    CodeAuthoringInputDescriptor {
        name: name.into(),
        kind: CodeAuthoringArgumentKind::Reference(kind),
        minimum: 1,
        maximum: 1,
        binding: CodeAuthoringInputBinding::Slot {
            slot: InputSlot::new(role, index),
        },
    }
}

fn optional_reference_slot(
    name: &str,
    role: InputRole,
    index: u16,
    kind: IntentPortKind,
) -> CodeAuthoringInputDescriptor {
    let mut descriptor = reference_slot(name, role, index, kind);
    descriptor.minimum = 0;
    descriptor
}

fn reference_pair(role: InputRole, kind: IntentPortKind) -> Vec<CodeAuthoringInputDescriptor> {
    vec![
        reference_slot("first", role, 0, kind),
        reference_slot("second", role, 1, kind),
    ]
}

fn role_collection(
    name: &str,
    role: InputRole,
    kind: IntentPortKind,
    minimum: u16,
    maximum: u16,
) -> CodeAuthoringInputDescriptor {
    CodeAuthoringInputDescriptor {
        name: name.into(),
        kind: CodeAuthoringArgumentKind::Collection(CodeAuthoringCollectionMember::Reference(kind)),
        minimum,
        maximum,
        binding: CodeAuthoringInputBinding::Role { role },
    }
}

fn dynamic_collection(
    name: &str,
    member: CodeAuthoringCollectionMember,
    schema: IntentChildSchema,
    minimum: u16,
    maximum: u16,
) -> CodeAuthoringInputDescriptor {
    CodeAuthoringInputDescriptor {
        name: name.into(),
        kind: CodeAuthoringArgumentKind::Collection(member),
        minimum,
        maximum,
        binding: CodeAuthoringInputBinding::DynamicChildren { schema },
    }
}

fn role_maximum(schema: &IntentNodeSchema, role: InputRole) -> u16 {
    schema
        .inputs
        .iter()
        .find(|input| input.role == role)
        .map_or(0, |input| input.maximum)
}

fn validate_input_coverage(
    family: &CodeAuthoringFamilyDescriptor,
    schema: &IntentNodeSchema,
    inputs: &[CodeAuthoringInputDescriptor],
) -> Result<(), CodeAuthoringCatalogError> {
    let mut names = BTreeSet::new();
    let mut covered_roles = BTreeSet::new();
    for input in inputs {
        if !names.insert(input.name.as_str()) {
            return Err(inconsistent(
                family,
                format!("duplicate named input `{}`", input.name),
            ));
        }
        match &input.binding {
            CodeAuthoringInputBinding::Slot { slot } => {
                let Some(cardinality) = schema
                    .inputs
                    .iter()
                    .find(|candidate| candidate.role == slot.role)
                else {
                    return Err(inconsistent(
                        family,
                        format!("named input `{}` maps an unsupported role", input.name),
                    ));
                };
                if slot.index >= cardinality.maximum {
                    return Err(inconsistent(
                        family,
                        format!("named input `{}` maps an out-of-range slot", input.name),
                    ));
                }
                covered_roles.insert(slot.role);
            }
            CodeAuthoringInputBinding::Role { role } => {
                covered_roles.insert(*role);
            }
            CodeAuthoringInputBinding::Roles { roles } => {
                covered_roles.extend(roles.iter().copied());
            }
            CodeAuthoringInputBinding::DynamicChildren { schema } => match schema {
                IntentChildSchema::PolylineVertex | IntentChildSchema::SplineControl => {
                    covered_roles.insert(InputRole::Point);
                }
                IntentChildSchema::FilletCorner => {
                    covered_roles.insert(InputRole::Span);
                }
                IntentChildSchema::PatternInstance | IntentChildSchema::None => {}
            },
        }
    }
    let expected_roles = schema
        .inputs
        .iter()
        .filter(|input| input.maximum > 0)
        .map(|input| input.role)
        .collect::<BTreeSet<_>>();
    if covered_roles != expected_roles {
        return Err(inconsistent(
            family,
            format!(
                "semantic input roles {covered_roles:?} do not cover central roles {expected_roles:?}"
            ),
        ));
    }
    for choice in &schema.input_choices {
        if !choice
            .alternatives
            .iter()
            .all(|slot| covered_roles.contains(&slot.role))
        {
            return Err(inconsistent(
                family,
                "one central cross-role input choice is absent from the semantic inputs",
            ));
        }
    }
    Ok(())
}

fn representative_outputs(
    family: &CodeAuthoringFamilyDescriptor,
    kind: &IntentNodeKind,
    dynamic_children: u16,
    fields: &[IntentDefinitionFieldDescriptor],
    schema: &IntentNodeSchema,
) -> Result<Vec<IntentDraftOutputDescriptor>, CodeAuthoringCatalogError> {
    let mut values = BTreeMap::new();
    for descriptor in fields {
        if let Some(value) = representative_field_value(descriptor, dynamic_children) {
            values.insert(descriptor.schema.field.clone(), value);
        }
    }
    build_output_descriptors(kind, dynamic_children, &values, schema, &[])
        .map_err(|reason| inconsistent(family, format!("output descriptor probe failed: {reason}")))
}

fn build_output_descriptors(
    kind: &IntentNodeKind,
    dynamic_children: u16,
    fields: &BTreeMap<IntentFieldKey, IntentLiteral>,
    schema: &IntentNodeSchema,
    operation_outputs: &[IntentOperationOutput],
) -> Result<Vec<IntentDraftOutputDescriptor>, String> {
    let symbol =
        IntentKey::new("catalogProbe").expect("source-controlled catalog probe symbol is valid");
    let mut draft = IntentNodeDraft::new(kind.clone(), symbol)
        .with_dynamic_children(dynamic_children)
        .with_operation_outputs(operation_outputs.to_vec());
    for (field, value) in fields {
        draft = draft.with_field(field.clone(), value.clone());
    }

    let mut alias_index = 0_u16;
    for cardinality in &schema.inputs {
        // Geometry point arguments are source samples even when the native
        // graph accepts a literal instead of an existing point reference.
        // Bind every concrete point slot in the catalog probe so output
        // descriptors retain the source-to-native alias coordinate needed by
        // reverse projection (for example Cubic `firstControl` ->
        // `controls[0]`). Other roles keep their true minimum cardinality.
        let probe_count = if matches!(kind, IntentNodeKind::Geometry { .. })
            && cardinality.role == InputRole::Point
        {
            cardinality.maximum
        } else {
            cardinality.minimum
        };
        for index in 0..probe_count {
            let slot = InputSlot::new(cardinality.role, index);
            draft = draft.with_input(slot, probe_alias(alias_index));
            alias_index = alias_index.saturating_add(1);
        }
    }
    for choice in &schema.input_choices {
        let present = choice
            .alternatives
            .iter()
            .filter(|slot| draft.inputs.contains_key(slot))
            .count();
        if present < usize::from(choice.minimum) {
            let Some(slot) = choice
                .alternatives
                .iter()
                .find(|slot| !draft.inputs.contains_key(slot))
                .copied()
            else {
                return Err("central input choice cannot be satisfied by its alternatives".into());
            };
            draft = draft.with_input(slot, probe_alias(alias_index));
        }
    }
    draft
        .output_descriptors()
        .map_err(|error| error.to_string())
}

fn probe_alias(index: u16) -> PatchPortRef {
    PatchPortRef::Alias {
        node: IntentKey::new(format!("catalogInput{index}"))
            .expect("bounded source-controlled catalog input alias is valid"),
        selector: IntentPortSelector::Node {
            role: IntentPortRole::Primary,
            index: 0,
        },
    }
}

fn representative_field_value(
    descriptor: &IntentDefinitionFieldDescriptor,
    dynamic_children: u16,
) -> Option<IntentLiteral> {
    match &descriptor.default {
        IntentFieldDefault::Literal(value) => return Some(value.clone()),
        IntentFieldDefault::Contextual | IntentFieldDefault::Conditional => return None,
        IntentFieldDefault::Required => {}
    }
    if let IntentFieldChoices::Closed(choices) = &descriptor.choices
        && let Some(choice) = choices.first()
    {
        return Some(IntentLiteral::Enum(choice.clone()));
    }
    let name = descriptor.schema.field.0.as_str();
    Some(match descriptor.schema.literal {
        IntentLiteralSchema::Boolean => IntentLiteral::Boolean(false),
        IntentLiteralSchema::Integer => IntentLiteral::Integer(0),
        IntentLiteralSchema::Natural if name == "degree" => {
            IntentLiteral::Natural(u64::from(dynamic_children.saturating_sub(1).clamp(1, 3)))
        }
        IntentLiteralSchema::Natural if name == "instances" => {
            IntentLiteral::Natural(u64::from(dynamic_children))
        }
        IntentLiteralSchema::Natural if name == "sides" => IntentLiteral::Natural(3),
        IntentLiteralSchema::Natural => IntentLiteral::Natural(1),
        IntentLiteralSchema::Text => IntentLiteral::Text(
            IntentKey::new("catalog").expect("source-controlled text literal is valid"),
        ),
        IntentLiteralSchema::Enum => IntentLiteral::Enum(
            IntentKey::new("catalog").expect("source-controlled enum probe is valid"),
        ),
        IntentLiteralSchema::Point => IntentLiteral::Point([0.0, 0.0]),
        IntentLiteralSchema::Quantity(unit) => IntentLiteral::Quantity { value: 1.0, unit },
    })
}

/// Descriptor-owned result shape shared by the Rust expansion layer and the
/// generated TypeScript semantic-reference surface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "snake_case", deny_unknown_fields)]
pub enum CodeResultShape {
    Leaf {
        kind: FeatureKind,
    },
    NativeSpan,
    Object {
        fields: BTreeMap<String, CodeResultShape>,
    },
    /// Intrinsically ordered fixed or argument-sized result members.
    Tuple {
        items: Vec<CodeResultShape>,
    },
    Keyed {
        kind: FeatureKind,
        derived_from_owner: bool,
    },
    NativeSpanKeyed {
        derived_from_owner: bool,
    },
    /// A clean named builder collection whose stable member keys are derived
    /// from one admitted authored argument, never from allocation order.
    DynamicKeyed {
        source: CodeResultKeySource,
        derived_from_owner: bool,
        member: Box<CodeResultShape>,
    },
}

/// Authenticated source of stable keys for one clean declaration result.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeResultKeySource {
    PolylineVertices,
    PolylineSegments,
    PolylineCorners,
    SplineControls,
    SplineSpans,
    FilletCorners,
}

/// One high-level declaration result exposed to managed/custom TypeScript.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeDeclarationResultDescriptor {
    pub feature_kind: FeatureKind,
    pub outputs: CodeResultShape,
}

/// Managed high-level result catalog.
///
/// This deliberately describes semantic result paths, not intent ports or
/// native IDs. The TypeScript source checked into the optional package is
/// generated byte-for-byte from this function and then mapped to branded
/// `OutputRef` values by its type system. New generated authoring interfaces
/// use [`resolve_code_authoring_declaration`] instead.
#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "the closed generated-result catalog stays auditable as one exhaustive table"
)]
pub fn declaration_result_catalog() -> BTreeMap<String, CodeDeclarationResultDescriptor> {
    let mut catalog = BTreeMap::from([
        (
            "computed.fillet".into(),
            descriptor(object([("arc", leaf(FeatureKind::CurveSpan))])),
        ),
        (
            "computed.roundedRectangleProfile".into(),
            descriptor(object([
                (
                    "mounts",
                    object([
                        ("ne", leaf(FeatureKind::Point)),
                        ("nw", leaf(FeatureKind::Point)),
                        ("se", leaf(FeatureKind::Point)),
                        ("sw", leaf(FeatureKind::Point)),
                    ]),
                ),
                ("profile", leaf(FeatureKind::Profile)),
            ])),
        ),
    ]);
    for family in &CODE_AUTHORING_FAMILIES {
        catalog.insert(
            format!("{}.{}", family.namespace, family.method),
            clean_authoring_result_descriptor(*family),
        );
    }
    catalog
}

#[allow(
    clippy::too_many_lines,
    reason = "one closed result table keeps the clean TypeScript surface visibly aligned with the native authoring inventory"
)]
fn clean_authoring_result_descriptor(
    family: CodeAuthoringFamilyDescriptor,
) -> CodeDeclarationResultDescriptor {
    use CodeAuthoringDeclarationKind as D;
    use ConstraintKind as C;
    use GeometryRecipeKind as G;
    match family.declaration {
        D::Geometry(recipe) => descriptor(match recipe {
            G::SketchPoint => object([("point", leaf(FeatureKind::Point))]),
            G::Segment => curve_shape([
                ("start", leaf(FeatureKind::Point)),
                ("end", leaf(FeatureKind::Point)),
            ]),
            G::Polyline => object([
                ("curve", leaf(FeatureKind::Curve)),
                (
                    "vertices",
                    dynamic_keyed(
                        CodeResultKeySource::PolylineVertices,
                        leaf(FeatureKind::Point),
                        false,
                    ),
                ),
                (
                    "segments",
                    dynamic_keyed(CodeResultKeySource::PolylineSegments, native_span(), false),
                ),
                (
                    "filletableCorners",
                    dynamic_keyed(
                        CodeResultKeySource::PolylineCorners,
                        leaf(FeatureKind::FeatureCorner),
                        true,
                    ),
                ),
            ]),
            G::MidpointLine => curve_shape([
                ("start", leaf(FeatureKind::Point)),
                ("midpoint", leaf(FeatureKind::Point)),
                ("end", leaf(FeatureKind::Point)),
                ("constraint", leaf(FeatureKind::Constraint)),
            ]),
            G::TwoPointAlignedRectangle | G::ThreePointCornerRectangle => rectangle_shape(false),
            G::CenterRectangle | G::ThreePointCenterRectangle => rectangle_shape(true),
            G::CenterRadiusCircle | G::TwoPointDiameterCircle | G::ThreePointCircle => {
                circle_shape()
            }
            G::CenterArc | G::ThreePointArc | G::TangentArc => arc_shape(),
            G::CenterAxesEllipse | G::AxisEndpointsEllipse => ellipse_shape(false),
            G::CenterAxesEllipticalArc | G::AxisEndpointsEllipticalArc => ellipse_shape(true),
            G::QuadraticBezier => curve_shape([
                ("start", leaf(FeatureKind::Point)),
                ("control", leaf(FeatureKind::Point)),
                ("end", leaf(FeatureKind::Point)),
            ]),
            G::CubicBezier => curve_shape([
                ("start", leaf(FeatureKind::Point)),
                (
                    "controls",
                    tuple([leaf(FeatureKind::Point), leaf(FeatureKind::Point)]),
                ),
                ("end", leaf(FeatureKind::Point)),
            ]),
            G::RationalQuadraticConic => curve_shape([
                ("start", leaf(FeatureKind::Point)),
                ("weightedMiddle", leaf(FeatureKind::Point)),
                ("end", leaf(FeatureKind::Point)),
                ("middleWeight", leaf(FeatureKind::Scalar)),
            ]),
            G::Parabola => curve_shape([
                ("vertex", leaf(FeatureKind::Point)),
                ("focus", leaf(FeatureKind::Point)),
                ("trimStart", leaf(FeatureKind::Scalar)),
                ("trimEnd", leaf(FeatureKind::Scalar)),
            ]),
            G::Hyperbola => curve_shape([
                ("center", leaf(FeatureKind::Point)),
                ("transverseAxisPoint", leaf(FeatureKind::Point)),
                ("semiConjugate", leaf(FeatureKind::Scalar)),
                ("trimStart", leaf(FeatureKind::Scalar)),
                ("trimEnd", leaf(FeatureKind::Scalar)),
            ]),
            G::OpenControlBSpline | G::PeriodicControlBSpline => object([
                ("curve", leaf(FeatureKind::Curve)),
                (
                    "controls",
                    dynamic_keyed(
                        CodeResultKeySource::SplineControls,
                        object([("position", leaf(FeatureKind::Point))]),
                        false,
                    ),
                ),
                (
                    "spans",
                    dynamic_keyed(CodeResultKeySource::SplineSpans, native_span(), false),
                ),
            ]),
            G::OpenControlNurbs | G::PeriodicControlNurbs => object([
                ("curve", leaf(FeatureKind::Curve)),
                (
                    "controls",
                    dynamic_keyed(
                        CodeResultKeySource::SplineControls,
                        object([
                            ("position", leaf(FeatureKind::Point)),
                            ("weight", leaf(FeatureKind::Scalar)),
                        ]),
                        false,
                    ),
                ),
                (
                    "spans",
                    dynamic_keyed(CodeResultKeySource::SplineSpans, native_span(), false),
                ),
            ]),
        }),
        D::Constraint(constraint) => {
            let outputs = if matches!(
                constraint,
                C::PointOnCurve | C::LineCurveTangency | C::CurveDirection
            ) {
                object([
                    ("constraint", leaf(FeatureKind::Constraint)),
                    ("contact", contact_shape()),
                ])
            } else if matches!(
                constraint,
                C::LineCircleTangency
                    | C::CircleArcTangency
                    | C::CurveCurveContact
                    | C::CurveCurveTangency
                    | C::EqualCurvature
                    | C::EndpointContinuity
                    | C::LineLineFillet
                    | C::CurveCurveFillet
            ) {
                object([
                    ("constraint", leaf(FeatureKind::Constraint)),
                    (
                        "contacts",
                        object([("first", contact_shape()), ("second", contact_shape())]),
                    ),
                ])
            } else {
                object([("constraint", leaf(FeatureKind::Constraint))])
            };
            descriptor_kind(FeatureKind::Constraint, outputs)
        }
        D::Dimension(_) => descriptor_kind(
            FeatureKind::Dimension,
            object([
                ("value", leaf(FeatureKind::Scalar)),
                ("dimension", leaf(FeatureKind::Dimension)),
            ]),
        ),
        D::Aggregate(AggregateKind::OpenChain) => descriptor_kind(
            FeatureKind::Chain,
            object([("chain", leaf(FeatureKind::Chain))]),
        ),
        D::Aggregate(AggregateKind::ClosedProfile) => descriptor_kind(
            FeatureKind::Profile,
            object([("profile", leaf(FeatureKind::Profile))]),
        ),
        D::ComputedFeature(ComputedFeatureKind::FilletSet) => descriptor(object([(
            "fillets",
            dynamic_keyed(
                CodeResultKeySource::FilletCorners,
                object([
                    ("corner", leaf(FeatureKind::FeatureCorner)),
                    ("arc", native_span()),
                ]),
                false,
            ),
        )])),
        D::Operation(operation) => {
            descriptor_kind(FeatureKind::Operation, operation_result_shape(operation))
        }
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one closed operation-result table keeps the complete stable public surface reviewable"
)]
fn operation_result_shape(operation: OperationKind) -> CodeResultShape {
    use OperationKind as O;
    let operation_leaf = ("operation", leaf(FeatureKind::Operation));
    match operation {
        O::Split => object([
            operation_leaf,
            ("before", native_span()),
            ("after", native_span()),
        ]),
        O::Break => object([
            operation_leaf,
            ("before", native_span()),
            ("middle", native_span()),
            ("after", native_span()),
        ]),
        O::Trim => object([operation_leaf, ("retained", native_span())]),
        O::Extend => object([
            operation_leaf,
            ("curve", leaf(FeatureKind::Curve)),
            ("span", native_span()),
        ]),
        O::Mirror => object([
            operation_leaf,
            ("curve", leaf(FeatureKind::Curve)),
            ("span", native_span()),
            ("controls", object([])),
            ("symmetryConstraints", object([])),
        ]),
        O::Chamfer => object([
            operation_leaf,
            (
                "endpoints",
                object([
                    ("first", leaf(FeatureKind::Point)),
                    ("second", leaf(FeatureKind::Point)),
                ]),
            ),
            ("edge", leaf(FeatureKind::Curve)),
            ("span", native_span()),
            (
                "parents",
                object([
                    ("first", associated_contact_shape()),
                    ("second", associated_contact_shape()),
                ]),
            ),
            (
                "distances",
                object([("first", dimension_shape()), ("second", dimension_shape())]),
            ),
        ]),
        O::AssociativeFillet => object([
            operation_leaf,
            ("center", leaf(FeatureKind::Point)),
            ("radius", leaf(FeatureKind::Scalar)),
            ("startAngle", leaf(FeatureKind::Scalar)),
            ("endAngle", leaf(FeatureKind::Scalar)),
            ("arc", leaf(FeatureKind::Curve)),
            ("span", native_span()),
            (
                "parents",
                object([("first", contact_shape()), ("second", contact_shape())]),
            ),
            ("association", leaf(FeatureKind::Constraint)),
            ("radiusDimension", dimension_shape()),
        ]),
        O::Rectangle => object([
            operation_leaf,
            (
                "corners",
                object([
                    ("bottomLeft", leaf(FeatureKind::Point)),
                    ("bottomRight", leaf(FeatureKind::Point)),
                    ("topRight", leaf(FeatureKind::Point)),
                    ("topLeft", leaf(FeatureKind::Point)),
                ]),
            ),
            (
                "edges",
                object([
                    ("bottom", leaf(FeatureKind::Curve)),
                    ("right", leaf(FeatureKind::Curve)),
                    ("top", leaf(FeatureKind::Curve)),
                    ("left", leaf(FeatureKind::Curve)),
                ]),
            ),
            (
                "spans",
                object([
                    ("bottom", native_span()),
                    ("right", native_span()),
                    ("top", native_span()),
                    ("left", native_span()),
                ]),
            ),
            ("constraints", object([])),
            (
                "dimensions",
                object([("width", dimension_shape()), ("height", dimension_shape())]),
            ),
        ]),
        O::RegularPolygon => object([
            operation_leaf,
            ("vertices", tuple([])),
            ("edges", tuple([])),
            ("spans", tuple([])),
        ]),
        O::Slot => object([
            operation_leaf,
            (
                "centers",
                object([
                    ("first", leaf(FeatureKind::Point)),
                    ("second", leaf(FeatureKind::Point)),
                ]),
            ),
            ("boundaryPoints", object([])),
            (
                "edges",
                object([
                    ("top", leaf(FeatureKind::Curve)),
                    ("bottom", leaf(FeatureKind::Curve)),
                ]),
            ),
            (
                "spans",
                object([("top", native_span()), ("bottom", native_span())]),
            ),
            (
                "arcs",
                object([
                    ("right", operation_arc_shape()),
                    ("left", operation_arc_shape()),
                ]),
            ),
            ("joins", object([])),
            ("fixedConstraints", object([])),
        ]),
        O::LinearPattern => object([operation_leaf, ("instances", tuple([]))]),
        O::ProfileOffset => object([
            operation_leaf,
            ("operand", object([])),
            ("distance", dimension_shape()),
        ]),
    }
}

fn contact_shape() -> CodeResultShape {
    object([
        ("parameter", leaf(FeatureKind::Scalar)),
        ("contact", leaf(FeatureKind::Contact)),
    ])
}

fn associated_contact_shape() -> CodeResultShape {
    object([
        ("parameter", leaf(FeatureKind::Scalar)),
        ("contact", leaf(FeatureKind::Contact)),
        ("constraint", leaf(FeatureKind::Constraint)),
    ])
}

fn dimension_shape() -> CodeResultShape {
    object([
        ("value", leaf(FeatureKind::Scalar)),
        ("dimension", leaf(FeatureKind::Dimension)),
    ])
}

fn operation_arc_shape() -> CodeResultShape {
    curve_shape([
        ("radius", leaf(FeatureKind::Scalar)),
        ("startAngle", leaf(FeatureKind::Scalar)),
        ("endAngle", leaf(FeatureKind::Scalar)),
    ])
}

fn curve_shape<const N: usize>(fields: [(&'static str, CodeResultShape); N]) -> CodeResultShape {
    let mut fields = fields.into_iter().collect::<BTreeMap<_, _>>();
    fields.insert("curve", leaf(FeatureKind::Curve));
    fields.insert("span", native_span());
    CodeResultShape::Object {
        fields: fields
            .into_iter()
            .map(|(name, shape)| (name.to_owned(), shape))
            .collect(),
    }
}

fn rectangle_shape(center: bool) -> CodeResultShape {
    let mut fields = BTreeMap::from([
        (
            "corners".to_owned(),
            tuple([
                leaf(FeatureKind::Point),
                leaf(FeatureKind::Point),
                leaf(FeatureKind::Point),
                leaf(FeatureKind::Point),
            ]),
        ),
        (
            "curves".to_owned(),
            tuple([
                leaf(FeatureKind::Curve),
                leaf(FeatureKind::Curve),
                leaf(FeatureKind::Curve),
                leaf(FeatureKind::Curve),
            ]),
        ),
        (
            "spans".to_owned(),
            tuple([native_span(), native_span(), native_span(), native_span()]),
        ),
    ]);
    if center {
        fields.insert("center".into(), leaf(FeatureKind::Point));
    }
    CodeResultShape::Object { fields }
}

fn circle_shape() -> CodeResultShape {
    object([
        ("center", leaf(FeatureKind::Point)),
        ("radius", leaf(FeatureKind::Scalar)),
        ("curve", leaf(FeatureKind::Curve)),
        ("span", native_span()),
    ])
}

fn arc_shape() -> CodeResultShape {
    object([
        ("center", leaf(FeatureKind::Point)),
        ("start", leaf(FeatureKind::Point)),
        ("midpoint", leaf(FeatureKind::Point)),
        ("end", leaf(FeatureKind::Point)),
        ("radius", leaf(FeatureKind::Scalar)),
        ("startAngle", leaf(FeatureKind::Scalar)),
        ("endAngle", leaf(FeatureKind::Scalar)),
        ("curve", leaf(FeatureKind::Curve)),
        ("span", native_span()),
    ])
}

fn ellipse_shape(arc: bool) -> CodeResultShape {
    let mut fields = BTreeMap::from([
        ("center".to_owned(), leaf(FeatureKind::Point)),
        ("majorAxisPoint".to_owned(), leaf(FeatureKind::Point)),
        ("minorAxisPoint".to_owned(), leaf(FeatureKind::Point)),
        ("minorAxisRatio".to_owned(), leaf(FeatureKind::Scalar)),
        ("curve".to_owned(), leaf(FeatureKind::Curve)),
        ("span".to_owned(), native_span()),
    ]);
    if arc {
        fields.extend([
            ("start".into(), leaf(FeatureKind::Point)),
            ("end".into(), leaf(FeatureKind::Point)),
            ("startAngle".into(), leaf(FeatureKind::Scalar)),
            ("endAngle".into(), leaf(FeatureKind::Scalar)),
        ]);
    }
    CodeResultShape::Object { fields }
}

/// Exact checked-in TypeScript catalog source.
///
/// # Panics
///
/// Panics only if the closed descriptor catalog cannot be serialized.
#[must_use]
pub fn typescript_declaration_result_catalog() -> String {
    let json = serde_json::to_string_pretty(&declaration_result_catalog())
        .expect("closed declaration result catalog is serializable");
    let authoring = CODE_AUTHORING_FAMILIES
        .iter()
        .map(|family| {
            (
                format!("{}.{}", family.namespace, family.method),
                family.availability,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let authoring_json = serde_json::to_string_pretty(&authoring)
        .expect("closed authoring method catalog is serializable");
    format!(
        concat!(
            "// SPDX-License-Identifier: GPL-3.0-or-later\n",
            "// @generated by geosolve-sketch-code::typescript_declaration_result_catalog.\n",
            "// Do not edit by hand.\n\n",
            "export const DECLARATION_RESULT_CATALOG = {} as const;\n\n",
            "export const AUTHORING_METHOD_CATALOG = {} as const;\n",
        ),
        json, authoring_json,
    )
}

fn descriptor(outputs: CodeResultShape) -> CodeDeclarationResultDescriptor {
    descriptor_kind(FeatureKind::Feature, outputs)
}

fn descriptor_kind(
    feature_kind: FeatureKind,
    outputs: CodeResultShape,
) -> CodeDeclarationResultDescriptor {
    CodeDeclarationResultDescriptor {
        feature_kind,
        outputs,
    }
}

fn leaf(kind: FeatureKind) -> CodeResultShape {
    CodeResultShape::Leaf { kind }
}

fn native_span() -> CodeResultShape {
    CodeResultShape::NativeSpan
}

fn tuple<const N: usize>(items: [CodeResultShape; N]) -> CodeResultShape {
    CodeResultShape::Tuple {
        items: items.into_iter().collect(),
    }
}

fn dynamic_keyed(
    source: CodeResultKeySource,
    member: CodeResultShape,
    derived_from_owner: bool,
) -> CodeResultShape {
    CodeResultShape::DynamicKeyed {
        source,
        derived_from_owner,
        member: Box::new(member),
    }
}

fn object<const N: usize>(fields: [(&'static str, CodeResultShape); N]) -> CodeResultShape {
    CodeResultShape::Object {
        fields: fields
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn concrete_outputs(
        namespace: &str,
        method: &str,
        dynamic_children: u16,
        fields: &BTreeMap<IntentFieldKey, IntentLiteral>,
        operation_outputs: &[IntentOperationOutput],
    ) -> Result<Vec<IntentDraftOutputDescriptor>, String> {
        let family = code_authoring_family(namespace, method)
            .ok_or_else(|| format!("unknown test method {namespace}.{method}"))?;
        let kind = family.declaration.intent_kind();
        build_output_descriptors(
            &kind,
            dynamic_children,
            fields,
            &kind.schema(dynamic_children),
            operation_outputs,
        )
    }

    fn representative_child_count(family: &CodeAuthoringFamilyDescriptor) -> u16 {
        match (family.dynamic_children, family.declaration) {
            (CodeAuthoringDynamicChildren::None, _) => 0,
            (
                CodeAuthoringDynamicChildren::SplineControls,
                CodeAuthoringDeclarationKind::Geometry(
                    GeometryRecipeKind::PeriodicControlBSpline
                    | GeometryRecipeKind::PeriodicControlNurbs,
                ),
            ) => 3,
            (
                CodeAuthoringDynamicChildren::PolylineVertices
                | CodeAuthoringDynamicChildren::SplineControls,
                _,
            ) => 2,
            (
                CodeAuthoringDynamicChildren::FilletCorners
                | CodeAuthoringDynamicChildren::PatternInstances,
                _,
            ) => 1,
        }
    }

    #[test]
    fn clean_authoring_inventory_is_exhaustive_unique_and_explicitly_gated() {
        assert_eq!(CODE_AUTHORING_FAMILIES.len(), 85);
        assert_eq!(public_code_authoring_families().count(), 83);

        let mut names = BTreeSet::new();
        let mut geometry = BTreeSet::new();
        let mut constraints = BTreeSet::new();
        let mut dimensions = BTreeSet::new();
        let mut operations = BTreeSet::new();
        let mut aggregates = BTreeSet::new();
        let mut computed = BTreeSet::new();
        let mut gated = BTreeSet::new();
        for family in &CODE_AUTHORING_FAMILIES {
            assert!(names.insert((family.namespace, family.method)));
            assert!(!family.method.contains('_'));
            assert!(family.method.chars().next().is_some_and(char::is_lowercase));
            assert_eq!(
                code_authoring_family(family.namespace, family.method),
                Some(family)
            );
            match family.declaration {
                CodeAuthoringDeclarationKind::Geometry(kind) => {
                    geometry.insert(kind);
                }
                CodeAuthoringDeclarationKind::Constraint(kind) => {
                    constraints.insert(kind);
                }
                CodeAuthoringDeclarationKind::Dimension(kind) => {
                    dimensions.insert(kind);
                }
                CodeAuthoringDeclarationKind::Operation(kind) => {
                    operations.insert(kind);
                }
                CodeAuthoringDeclarationKind::Aggregate(kind) => {
                    aggregates.insert(kind);
                }
                CodeAuthoringDeclarationKind::ComputedFeature(kind) => {
                    computed.insert(kind);
                }
            }
            if family.availability == CodeAuthoringAvailability::RequiresHostSnapshot {
                gated.insert(family.declaration);
            }
        }
        assert_eq!(geometry, GeometryRecipeKind::ALL.into_iter().collect());
        assert_eq!(constraints, ConstraintKind::ALL.into_iter().collect());
        assert_eq!(dimensions, DimensionKind::ALL.into_iter().collect());
        assert_eq!(operations, OperationKind::ALL.into_iter().collect());
        assert_eq!(aggregates, AggregateKind::ALL.into_iter().collect());
        assert_eq!(computed, ComputedFeatureKind::ALL.into_iter().collect());
        assert_eq!(
            gated,
            BTreeSet::from([
                CodeAuthoringDeclarationKind::Constraint(ConstraintKind::ExternalPointCoincident),
                CodeAuthoringDeclarationKind::Constraint(ConstraintKind::ExternalLineCollinear),
            ])
        );
    }

    #[test]
    fn every_clean_method_resolves_through_central_fields_and_outputs() {
        for family in &CODE_AUTHORING_FAMILIES {
            let dynamic_children = representative_child_count(family);
            let descriptor = resolve_code_authoring_declaration(
                family.namespace,
                family.method,
                dynamic_children,
            )
            .unwrap_or_else(|error| panic!("{}.{}: {error}", family.namespace, family.method));
            assert_eq!(descriptor.declaration, family.declaration);
            assert_eq!(
                descriptor.fields,
                family
                    .declaration
                    .intent_kind()
                    .field_descriptors(dynamic_children)
            );
            assert!(!descriptor.outputs.is_empty());
            assert_eq!(descriptor.result_policy, family.result_policy);
            let expected_values = match family.declaration {
                CodeAuthoringDeclarationKind::Dimension(_)
                | CodeAuthoringDeclarationKind::Geometry(
                    GeometryRecipeKind::CenterRadiusCircle
                    | GeometryRecipeKind::RationalQuadraticConic,
                ) => 1,
                CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::Parabola) => 2,
                CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::Hyperbola) => 3,
                CodeAuthoringDeclarationKind::Geometry(_)
                | CodeAuthoringDeclarationKind::Constraint(_)
                | CodeAuthoringDeclarationKind::Operation(_)
                | CodeAuthoringDeclarationKind::Aggregate(_)
                | CodeAuthoringDeclarationKind::ComputedFeature(_) => 0,
            };
            assert_eq!(descriptor.values.len(), expected_values);
        }
    }

    #[test]
    fn ergonomic_geometry_and_constraint_inputs_are_named_not_transport_tuples() {
        let bezier = resolve_code_authoring_declaration("geometry", "quadraticBezier", 0).unwrap();
        assert_eq!(
            bezier
                .inputs
                .iter()
                .map(|input| input.name.as_str())
                .collect::<Vec<_>>(),
            ["start", "control", "end"]
        );
        assert!(
            bezier
                .inputs
                .iter()
                .all(|input| input.kind == CodeAuthoringArgumentKind::Point)
        );

        let circle =
            resolve_code_authoring_declaration("geometry", "centerRadiusCircle", 0).unwrap();
        assert_eq!(
            circle
                .inputs
                .iter()
                .map(|input| input.name.as_str())
                .collect::<Vec<_>>(),
            ["center"]
        );
        assert_eq!(
            circle.values[0].path,
            IntentProjectionPath::field(IntentKey::new("radius").unwrap())
        );

        let parallel = resolve_code_authoring_declaration("constraint", "parallel", 0).unwrap();
        assert_eq!(
            parallel
                .inputs
                .iter()
                .map(|input| input.name.as_str())
                .collect::<Vec<_>>(),
            ["first", "second"]
        );
        assert!(parallel.inputs.iter().all(|input| {
            input.kind == CodeAuthoringArgumentKind::Reference(IntentPortKind::CurveSpan)
        }));
    }

    #[test]
    fn typed_defaults_choices_and_dynamic_shapes_are_central_descriptor_owned() {
        let circle =
            resolve_code_authoring_declaration("geometry", "centerRadiusCircle", 0).unwrap();
        let role = circle
            .fields
            .iter()
            .find(|field| field.schema.field.0.as_str() == "role")
            .unwrap();
        assert_eq!(
            role.default,
            IntentFieldDefault::Literal(IntentLiteral::Enum(IntentKey::new("profile").unwrap()))
        );
        assert_eq!(
            role.choices,
            IntentFieldChoices::Closed(vec![
                IntentKey::new("profile").unwrap(),
                IntentKey::new("construction").unwrap(),
            ])
        );

        let fillets = resolve_code_authoring_declaration("computed", "filletSet", 2).unwrap();
        assert_eq!(fillets.dynamic_children.count, 2);
        assert_eq!(fillets.inputs[0].name, "corners");
        assert_eq!(
            fillets.inputs[0].kind,
            CodeAuthoringArgumentKind::Collection(CodeAuthoringCollectionMember::FilletCorner)
        );
        let radius = fillets
            .fields
            .iter()
            .find(|field| field.schema.field.0.as_str() == "radius")
            .unwrap();
        assert_eq!(
            radius.schema.literal,
            IntentLiteralSchema::Quantity(IntentUnit::Length)
        );
        assert_eq!(radius.default, IntentFieldDefault::Required);
        assert!(fillets.fields.iter().any(|field| {
            field.path.segments().iter().any(|segment| {
                matches!(
                    segment,
                    geosolve_sketch_intent::IntentProjectionPathSegment::Index(1)
                )
            })
        }));
    }

    #[test]
    fn concrete_field_and_authenticated_operation_shapes_regenerate_results() {
        let baseline = concrete_outputs(
            "geometry",
            "twoPointAlignedRectangle",
            0,
            &BTreeMap::new(),
            &[],
        )
        .unwrap();
        let regularized = concrete_outputs(
            "geometry",
            "twoPointAlignedRectangle",
            0,
            &BTreeMap::from([(
                IntentFieldKey(IntentKey::new("regularized").unwrap()),
                IntentLiteral::Boolean(true),
            )]),
            &[],
        )
        .unwrap();
        assert_eq!(regularized.len(), baseline.len() + 2);

        let mirror = concrete_outputs(
            "operation",
            "mirror",
            0,
            &BTreeMap::new(),
            &[IntentOperationOutput::curve(1)],
        )
        .unwrap();
        assert!(mirror.iter().any(|output| {
            output.kind == IntentPortKind::Curve && !output.path.segments().is_empty()
        }));
        assert!(mirror.iter().any(|output| {
            output.kind == IntentPortKind::CurveSpan && !output.path.segments().is_empty()
        }));
    }

    #[test]
    fn invalid_dynamic_cardinality_and_unknown_methods_fail_closed() {
        assert!(matches!(
            resolve_code_authoring_declaration("geometry", "polyline", 1),
            Err(CodeAuthoringCatalogError::InvalidDynamicChildren { .. })
        ));
        assert!(matches!(
            resolve_code_authoring_declaration("computed", "filletSet", 0),
            Err(CodeAuthoringCatalogError::InvalidDynamicChildren { .. })
        ));
        assert!(matches!(
            resolve_code_authoring_declaration("geometry", "missing", 0),
            Err(CodeAuthoringCatalogError::UnknownMethod { .. })
        ));
    }
}
