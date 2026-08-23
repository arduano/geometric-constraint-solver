// SPDX-License-Identifier: GPL-3.0-or-later

//! Closed input and definition-field schemas for projectional declarations.
//!
//! These tables describe structure only. They contain no residual, geometric
//! construction, tolerance, or solver policy.

use serde::{Deserialize, Serialize};

use crate::model::{MAX_INTENT_NODE_CHILDREN, MAX_INTENT_NODE_INPUTS};
use crate::{
    AggregateKind, ComputedFeatureKind, ConstraintKind, DimensionKind, ExternalIntentKind,
    GeometryRecipeKind, InputRole, InputSlot, IntentChildSchema, IntentFieldKey, IntentKey,
    IntentLiteral, IntentNodeKind, IntentUnit, OperationKind, ParameterIntentKind,
};

#[allow(
    clippy::cast_possible_truncation,
    reason = "the source-controlled resource bound is 4,096 and compile-time schema indices are u16"
)]
const MAX_SCHEMA_CHILDREN: u16 = MAX_INTENT_NODE_CHILDREN as u16;
#[allow(
    clippy::cast_possible_truncation,
    reason = "the source-controlled resource bound is 4,096 and compile-time schema indices are u16"
)]
const MAX_SCHEMA_INPUTS: u16 = MAX_INTENT_NODE_INPUTS as u16;
const MAX_FILLET_CORNERS: u16 = MAX_SCHEMA_INPUTS / 2;

/// Closed literal shape accepted by one definition field.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "unit", rename_all = "snake_case")]
pub enum IntentLiteralSchema {
    Boolean,
    Integer,
    Natural,
    Text,
    Enum,
    Point,
    Quantity(IntentUnit),
}

impl IntentLiteralSchema {
    pub(crate) fn accepts(self, value: &IntentLiteral) -> bool {
        matches!(
            (self, value),
            (Self::Boolean, IntentLiteral::Boolean(_))
                | (Self::Integer, IntentLiteral::Integer(_))
                | (Self::Natural, IntentLiteral::Natural(_))
                | (Self::Text, IntentLiteral::Text(_))
                | (Self::Enum, IntentLiteral::Enum(_))
                | (Self::Point, IntentLiteral::Point(_))
        ) || matches!(
            (self, value),
            (
                Self::Quantity(expected),
                IntentLiteral::Quantity { unit: actual, .. }
            ) if expected == *actual
        )
    }
}

/// Contiguous indexed cardinality for one typed dependency role.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentInputCardinality {
    pub role: InputRole,
    pub minimum: u16,
    pub maximum: u16,
}

/// Cross-role exact choice, used where the native catalog admits one of
/// several typed operand kinds (for example a face profile or an open chain).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentInputChoiceSchema {
    pub alternatives: Vec<InputSlot>,
    pub minimum: u16,
    pub maximum: u16,
}

/// Exact key and literal/unit contract for one definition field.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentDefinitionFieldSchema {
    pub field: IntentFieldKey,
    pub literal: IntentLiteralSchema,
    pub required: bool,
}

/// Complete structural schema generated for one declaration family.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentNodeSchema {
    pub inputs: Vec<IntentInputCardinality>,
    pub input_choices: Vec<IntentInputChoiceSchema>,
    pub fields: Vec<IntentDefinitionFieldSchema>,
    pub minimum_children: u16,
    pub maximum_children: u16,
}

impl IntentNodeSchema {
    pub(crate) fn input(&self, role: InputRole) -> Option<IntentInputCardinality> {
        self.inputs.iter().find(|entry| entry.role == role).copied()
    }

    pub(crate) fn field(&self, field: &IntentFieldKey) -> Option<&IntentDefinitionFieldSchema> {
        self.fields.iter().find(|entry| &entry.field == field)
    }
}

impl IntentNodeKind {
    /// Generates the exact structural schema for this declaration and its
    /// declared variable-cardinality children.
    #[must_use]
    pub fn schema(&self, dynamic_children: u16) -> IntentNodeSchema {
        node_schema(self, dynamic_children)
    }
}

fn field(name: &str, literal: IntentLiteralSchema, required: bool) -> IntentDefinitionFieldSchema {
    IntentDefinitionFieldSchema {
        field: IntentFieldKey(
            IntentKey::new(name).expect("source-controlled intent field keys are valid"),
        ),
        literal,
        required,
    }
}

const fn input(role: InputRole, minimum: u16, maximum: u16) -> IntentInputCardinality {
    IntentInputCardinality {
        role,
        minimum,
        maximum,
    }
}

fn choice(
    alternatives: &[(InputRole, u16)],
    minimum: u16,
    maximum: u16,
) -> IntentInputChoiceSchema {
    IntentInputChoiceSchema {
        alternatives: alternatives
            .iter()
            .map(|(role, index)| InputSlot::new(*role, *index))
            .collect(),
        minimum,
        maximum,
    }
}

fn schema(
    inputs: Vec<IntentInputCardinality>,
    input_choices: Vec<IntentInputChoiceSchema>,
    fields: Vec<IntentDefinitionFieldSchema>,
    child_bounds: (u16, u16),
) -> IntentNodeSchema {
    IntentNodeSchema {
        inputs,
        input_choices,
        fields,
        minimum_children: child_bounds.0,
        maximum_children: child_bounds.1,
    }
}

fn node_schema(kind: &IntentNodeKind, dynamic_children: u16) -> IntentNodeSchema {
    match kind {
        IntentNodeKind::Geometry { recipe } => geometry_schema(*recipe, dynamic_children),
        IntentNodeKind::Constraint { constraint } => constraint_schema(*constraint),
        IntentNodeKind::Dimension { dimension } => dimension_schema(*dimension),
        IntentNodeKind::Operation { operation } => operation_schema(*operation, dynamic_children),
        IntentNodeKind::ComputedFeature { feature } => {
            computed_feature_schema(*feature, dynamic_children)
        }
        IntentNodeKind::Aggregate { aggregate } => aggregate_schema(*aggregate),
        IntentNodeKind::Parameter { parameter } => parameter_schema(*parameter),
        IntentNodeKind::External { external } => external_schema(*external),
        // A bootstrap payload's public-domain codec owns its object-specific
        // cardinality. The intent layer still rejects fields and enforces the
        // closed role inventory in IntentNodeKind::accepts_input.
        IntentNodeKind::Bootstrap { .. } => schema(Vec::new(), Vec::new(), Vec::new(), (0, 0)),
        IntentNodeKind::Annotation => schema(
            vec![
                input(InputRole::Constraint, 0, 1),
                input(InputRole::Dimension, 0, 1),
                input(InputRole::Point, 0, 1),
                input(InputRole::Curve, 0, 1),
            ],
            vec![choice(
                &[(InputRole::Constraint, 0), (InputRole::Dimension, 0)],
                0,
                1,
            )],
            vec![field("offset", IntentLiteralSchema::Point, false)],
            (0, 0),
        ),
        IntentNodeKind::Identity { .. } => schema(
            vec![input(InputRole::Identity, 1, 1)],
            Vec::new(),
            Vec::new(),
            (0, 0),
        ),
    }
}

fn aggregate_schema(kind: AggregateKind) -> IntentNodeSchema {
    match kind {
        AggregateKind::OpenChain | AggregateKind::ClosedProfile => schema(
            vec![input(InputRole::Span, 1, MAX_SCHEMA_INPUTS)],
            Vec::new(),
            Vec::new(),
            (0, 0),
        ),
    }
}

fn geometry_schema(recipe: GeometryRecipeKind, dynamic_children: u16) -> IntentNodeSchema {
    use GeometryRecipeKind as G;
    let point_maximum = match recipe {
        G::SketchPoint => 1,
        G::Segment
        | G::MidpointLine
        | G::CenterRadiusCircle
        | G::TwoPointDiameterCircle
        | G::TangentArc
        | G::Parabola
        | G::Hyperbola
        | G::TwoPointAlignedRectangle
        | G::CenterRectangle => 2,
        G::ThreePointCornerRectangle
        | G::ThreePointCenterRectangle
        | G::ThreePointCircle
        | G::CenterArc
        | G::ThreePointArc
        | G::CenterAxesEllipse
        | G::AxisEndpointsEllipse
        | G::QuadraticBezier
        | G::RationalQuadraticConic => 3,
        G::CubicBezier => 4,
        G::CenterAxesEllipticalArc | G::AxisEndpointsEllipticalArc => 5,
        G::Polyline | G::OpenControlNurbs | G::PeriodicControlNurbs => dynamic_children,
    };
    let child_bounds = match recipe.child_schema() {
        IntentChildSchema::SplineControl if recipe == G::PeriodicControlNurbs => {
            (3, MAX_SCHEMA_CHILDREN)
        }
        IntentChildSchema::PolylineVertex | IntentChildSchema::SplineControl => {
            (2, MAX_SCHEMA_CHILDREN)
        }
        _ => (0, 0),
    };
    let mut inputs = vec![input(InputRole::Point, 0, point_maximum)];
    if recipe == G::TangentArc {
        inputs.push(input(InputRole::Span, 1, 1));
    }
    let mut fields = Vec::new();
    if matches!(recipe, G::Segment | G::MidpointLine) {
        fields.push(field("branch_direction", IntentLiteralSchema::Point, false));
    }
    if recipe == G::Polyline {
        fields.push(field("closed", IntentLiteralSchema::Boolean, false));
    }
    if matches!(
        recipe,
        G::TwoPointAlignedRectangle
            | G::ThreePointCornerRectangle
            | G::CenterRectangle
            | G::ThreePointCenterRectangle
    ) {
        fields.push(field("regularized", IntentLiteralSchema::Boolean, false));
    }
    if recipe == G::ThreePointCenterRectangle {
        // The side-midpoint click is an authoring sample rather than a
        // persistent sketch point. Retaining it as typed recipe state avoids
        // fabricating a native identity while keeping cold construction exact.
        fields.push(field("side_midpoint", IntentLiteralSchema::Point, false));
    }
    if matches!(
        recipe,
        G::CenterArc
            | G::ThreePointArc
            | G::TangentArc
            | G::CenterAxesEllipticalArc
            | G::AxisEndpointsEllipticalArc
    ) {
        fields.push(field("sweep", IntentLiteralSchema::Enum, false));
    }
    if recipe == G::RationalQuadraticConic {
        fields.push(field("weighted_middle", IntentLiteralSchema::Point, false));
    }
    if recipe == G::Hyperbola {
        fields.push(field("branch", IntentLiteralSchema::Enum, false));
    }
    if recipe == G::TangentArc {
        // These fields are the complete existing contact-cell state selected
        // by M78 authoring. Optional fields have exact catalog defaults; the
        // host validates the combinations for bounded/local/periodic forms.
        fields.extend([
            field(
                "source_parameter",
                IntentLiteralSchema::Quantity(IntentUnit::Dimensionless),
                false,
            ),
            field("source_winding", IntentLiteralSchema::Integer, false),
            field("source_domain", IntentLiteralSchema::Enum, false),
            field(
                "source_domain_lower",
                IntentLiteralSchema::Quantity(IntentUnit::Dimensionless),
                false,
            ),
            field(
                "source_domain_upper",
                IntentLiteralSchema::Quantity(IntentUnit::Dimensionless),
                false,
            ),
            field(
                "source_domain_period",
                IntentLiteralSchema::Quantity(IntentUnit::Dimensionless),
                false,
            ),
            field("source_neighborhood", IntentLiteralSchema::Enum, false),
            field(
                "source_neighborhood_lower",
                IntentLiteralSchema::Quantity(IntentUnit::Dimensionless),
                false,
            ),
            field(
                "source_neighborhood_upper",
                IntentLiteralSchema::Quantity(IntentUnit::Dimensionless),
                false,
            ),
            field("orientation", IntentLiteralSchema::Enum, false),
        ]);
    }
    if matches!(recipe, G::OpenControlNurbs | G::PeriodicControlNurbs) {
        fields.extend([
            field("degree", IntentLiteralSchema::Natural, false),
            field("gauge_index", IntentLiteralSchema::Natural, false),
        ]);
    }
    schema(inputs, Vec::new(), fields, child_bounds)
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive table keeps all persistent relation operand schemas reviewable"
)]
fn constraint_schema(kind: ConstraintKind) -> IntentNodeSchema {
    use ConstraintKind as C;
    let mut inputs = Vec::new();
    let mut fields = Vec::new();
    let mut add = |role, count| inputs.push(input(role, count, count));
    match kind {
        C::FixedPoint => {
            add(InputRole::Point, 1);
            fields.push(field("target", IntentLiteralSchema::Point, false));
        }
        C::FixedCoordinate => {
            add(InputRole::Point, 1);
            fields.extend([
                field("axis", IntentLiteralSchema::Enum, false),
                field(
                    "target",
                    IntentLiteralSchema::Quantity(IntentUnit::Length),
                    false,
                ),
            ]);
        }
        C::CoincidentWithOrigin => add(InputRole::Point, 1),
        C::PointOnDatumAxis => {
            add(InputRole::Point, 1);
            fields.push(field("axis", IntentLiteralSchema::Enum, false));
        }
        C::Coincident | C::HorizontalPoints | C::VerticalPoints => add(InputRole::Point, 2),
        C::ExternalPointCoincident => {
            add(InputRole::Point, 1);
            add(InputRole::External, 1);
        }
        C::Horizontal | C::Vertical => add(InputRole::Span, 1),
        C::HorizontalPointToMidpoint
        | C::VerticalPointToMidpoint
        | C::Midpoint
        | C::PointOnCurve => {
            add(InputRole::Point, 1);
            add(InputRole::Span, 1);
        }
        C::Parallel | C::Perpendicular | C::EqualLength => add(InputRole::Span, 2),
        C::ExternalLineCollinear => {
            add(InputRole::Span, 1);
            add(InputRole::External, 1);
            fields.push(field("direction", IntentLiteralSchema::Enum, false));
        }
        C::CollinearWithDatumAxis => {
            add(InputRole::Span, 1);
            fields.extend([
                field("axis", IntentLiteralSchema::Enum, false),
                field("direction", IntentLiteralSchema::Enum, false),
            ]);
        }
        C::Concentric | C::EqualRadius => add(InputRole::Curve, 2),
        C::Collinear => {
            add(InputRole::Span, 2);
            fields.extend([
                field("first_direction", IntentLiteralSchema::Enum, false),
                field("second_direction", IntentLiteralSchema::Enum, false),
            ]);
        }
        C::SymmetricAboutLine => {
            add(InputRole::Point, 2);
            add(InputRole::Span, 1);
        }
        C::SymmetricAboutDatumAxis => {
            add(InputRole::Point, 2);
            fields.push(field("axis", IntentLiteralSchema::Enum, false));
        }
        C::LineCircleTangency => {
            add(InputRole::Span, 1);
            add(InputRole::Curve, 1);
            fields.push(field("side", IntentLiteralSchema::Enum, false));
        }
        C::CircleCircleTangency => {
            add(InputRole::Curve, 2);
            fields.extend([
                field("mode", IntentLiteralSchema::Enum, false),
                field("center_direction", IntentLiteralSchema::Point, false),
            ]);
        }
        C::CircleArcTangency => {
            add(InputRole::Curve, 2);
            fields.push(field("side", IntentLiteralSchema::Enum, false));
        }
        C::LineCurveTangency | C::CurveCurveContact | C::CurveCurveTangency => {
            add(InputRole::Span, 2);
            if kind == C::LineCurveTangency {
                fields.push(field("endpoint", IntentLiteralSchema::Enum, true));
            }
        }
        C::CurveDirection => {
            add(InputRole::Span, 2);
            fields.extend([
                field("relation", IntentLiteralSchema::Enum, true),
                field("orientation", IntentLiteralSchema::Enum, false),
                field("side", IntentLiteralSchema::Enum, false),
            ]);
        }
        C::EqualCurvature => {
            add(InputRole::Span, 2);
            fields.push(field("relation", IntentLiteralSchema::Enum, true));
        }
        C::EndpointContinuity => {
            add(InputRole::Span, 2);
            fields.extend([
                field("continuity", IntentLiteralSchema::Enum, true),
                field(
                    "parameter_ratio",
                    IntentLiteralSchema::Quantity(IntentUnit::Dimensionless),
                    false,
                ),
            ]);
        }
        C::LineLineFillet | C::CurveCurveFillet => {
            add(InputRole::Curve, 1);
            add(InputRole::Span, 2);
            fields.extend([
                field("first_side", IntentLiteralSchema::Enum, true),
                field("second_side", IntentLiteralSchema::Enum, true),
                field("endpoint_order", IntentLiteralSchema::Enum, true),
            ]);
            if kind == C::CurveCurveFillet {
                fields.extend([
                    field("first_trim_endpoint", IntentLiteralSchema::Enum, true),
                    field("second_trim_endpoint", IntentLiteralSchema::Enum, true),
                ]);
            }
        }
    }
    schema(inputs, Vec::new(), fields, (0, 0))
}

fn dimension_schema(kind: DimensionKind) -> IntentNodeSchema {
    use DimensionKind as D;
    let inputs = match kind {
        D::PointDistance => vec![input(InputRole::Point, 2, 2)],
        D::CurveLength => vec![input(InputRole::Span, 1, 1)],
        D::Radius | D::Diameter => vec![input(InputRole::Curve, 1, 1)],
        D::OrientedAngle | D::SupportingLineOffset | D::ExactTranslatedSegmentOffset => {
            vec![input(InputRole::Span, 2, 2)]
        }
        D::ProfileOffset => vec![
            input(InputRole::Profile, 0, 1),
            input(InputRole::Chain, 0, 1),
        ],
    };
    let mut choices = Vec::new();
    if kind == D::ProfileOffset {
        choices.push(choice(
            &[(InputRole::Profile, 0), (InputRole::Chain, 0)],
            1,
            1,
        ));
    }
    let mut fields = vec![field("mode", IntentLiteralSchema::Enum, false)];
    match kind {
        D::OrientedAngle => fields.push(field("orientation", IntentLiteralSchema::Enum, false)),
        D::SupportingLineOffset | D::ExactTranslatedSegmentOffset => fields.extend([
            field("side", IntentLiteralSchema::Enum, false),
            field("orientation", IntentLiteralSchema::Enum, false),
        ]),
        D::ProfileOffset => fields.extend([
            field("direction", IntentLiteralSchema::Enum, false),
            field("side", IntentLiteralSchema::Enum, false),
        ]),
        D::PointDistance | D::CurveLength | D::Radius | D::Diameter => {}
    }
    schema(inputs, choices, fields, (0, 0))
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive table mirrors the existing equation-free operation request catalog"
)]
fn operation_schema(kind: OperationKind, dynamic_children: u16) -> IntentNodeSchema {
    use OperationKind as O;
    let (inputs, fields, child_bounds) = match kind {
        O::Split | O::Trim => (
            vec![input(InputRole::Span, 1, 1)],
            vec![
                field(
                    "parameter",
                    IntentLiteralSchema::Quantity(IntentUnit::Dimensionless),
                    true,
                ),
                field("retained", IntentLiteralSchema::Enum, true),
            ],
            (0, 0),
        ),
        O::Break => (
            vec![input(InputRole::Span, 1, 1)],
            vec![
                field(
                    "start",
                    IntentLiteralSchema::Quantity(IntentUnit::Dimensionless),
                    true,
                ),
                field(
                    "end",
                    IntentLiteralSchema::Quantity(IntentUnit::Dimensionless),
                    true,
                ),
                field("retained", IntentLiteralSchema::Enum, true),
            ],
            (0, 0),
        ),
        O::Extend => (
            vec![input(InputRole::Span, 2, 2)],
            vec![field("endpoint", IntentLiteralSchema::Enum, true)],
            (0, 0),
        ),
        O::Mirror => (
            vec![input(InputRole::Curve, 1, 1), input(InputRole::Span, 1, 1)],
            Vec::new(),
            (0, 0),
        ),
        O::Chamfer => (
            vec![input(InputRole::Span, 2, 2)],
            vec![
                field(
                    "first_distance",
                    IntentLiteralSchema::Quantity(IntentUnit::Length),
                    true,
                ),
                field(
                    "second_distance",
                    IntentLiteralSchema::Quantity(IntentUnit::Length),
                    true,
                ),
            ],
            (0, 0),
        ),
        O::AssociativeFillet => (
            vec![input(InputRole::Span, 2, 2)],
            vec![field(
                "radius",
                IntentLiteralSchema::Quantity(IntentUnit::Length),
                true,
            )],
            (0, 0),
        ),
        O::Rectangle => (
            Vec::new(),
            vec![
                field("origin", IntentLiteralSchema::Point, true),
                field(
                    "width",
                    IntentLiteralSchema::Quantity(IntentUnit::Length),
                    true,
                ),
                field(
                    "height",
                    IntentLiteralSchema::Quantity(IntentUnit::Length),
                    true,
                ),
            ],
            (0, 0),
        ),
        O::RegularPolygon => (
            Vec::new(),
            vec![
                field("center", IntentLiteralSchema::Point, true),
                field(
                    "radius",
                    IntentLiteralSchema::Quantity(IntentUnit::Length),
                    true,
                ),
                field("sides", IntentLiteralSchema::Natural, true),
                field(
                    "rotation",
                    IntentLiteralSchema::Quantity(IntentUnit::Angle),
                    true,
                ),
            ],
            (0, 0),
        ),
        O::Slot => (
            Vec::new(),
            vec![
                field("first_center", IntentLiteralSchema::Point, true),
                field("second_center", IntentLiteralSchema::Point, true),
                field(
                    "radius",
                    IntentLiteralSchema::Quantity(IntentUnit::Length),
                    true,
                ),
            ],
            (0, 0),
        ),
        O::LinearPattern => (
            vec![input(InputRole::Curve, 1, MAX_SCHEMA_INPUTS)],
            vec![
                field("instances", IntentLiteralSchema::Natural, true),
                field("step", IntentLiteralSchema::Point, true),
            ],
            (1, MAX_SCHEMA_CHILDREN),
        ),
        O::ProfileOffset => (
            vec![
                input(InputRole::Profile, 0, 1),
                input(InputRole::Chain, 0, 1),
            ],
            vec![
                field(
                    "distance",
                    IntentLiteralSchema::Quantity(IntentUnit::Length),
                    true,
                ),
                field("direction", IntentLiteralSchema::Enum, false),
                field("side", IntentLiteralSchema::Enum, false),
            ],
            (0, 0),
        ),
    };
    let choices = if kind == O::ProfileOffset {
        vec![choice(
            &[(InputRole::Profile, 0), (InputRole::Chain, 0)],
            1,
            1,
        )]
    } else {
        Vec::new()
    };
    let _ = dynamic_children;
    schema(inputs, choices, fields, child_bounds)
}

fn computed_feature_schema(kind: ComputedFeatureKind, dynamic_children: u16) -> IntentNodeSchema {
    match kind {
        ComputedFeatureKind::FilletSet => schema(
            vec![input(
                InputRole::Span,
                dynamic_children.saturating_mul(2),
                dynamic_children.saturating_mul(2),
            )],
            Vec::new(),
            vec![field(
                "radius",
                IntentLiteralSchema::Quantity(IntentUnit::Length),
                true,
            )],
            (1, MAX_FILLET_CORNERS),
        ),
    }
}

fn parameter_schema(kind: ParameterIntentKind) -> IntentNodeSchema {
    match kind {
        ParameterIntentKind::Parameter => schema(
            Vec::new(),
            Vec::new(),
            vec![field("kind", IntentLiteralSchema::Enum, true)],
            (0, 0),
        ),
        ParameterIntentKind::Binding => schema(
            vec![
                input(InputRole::Parameter, 1, 1),
                input(InputRole::Dimension, 0, 1),
                input(InputRole::Scalar, 0, 1),
                input(InputRole::Source, 0, 1),
            ],
            vec![choice(
                &[
                    (InputRole::Dimension, 0),
                    (InputRole::Scalar, 0),
                    (InputRole::Source, 0),
                ],
                1,
                1,
            )],
            Vec::new(),
            (0, 0),
        ),
        ParameterIntentKind::Output => schema(
            vec![
                input(InputRole::Parameter, 1, 1),
                input(InputRole::Dimension, 1, 1),
            ],
            Vec::new(),
            Vec::new(),
            (0, 0),
        ),
    }
}

fn external_schema(kind: ExternalIntentKind) -> IntentNodeSchema {
    match kind {
        ExternalIntentKind::Binding => schema(
            Vec::new(),
            Vec::new(),
            vec![field("codec", IntentLiteralSchema::Text, true)],
            (0, 0),
        ),
        ExternalIntentKind::SnapshotReference => schema(
            vec![input(InputRole::External, 1, 1)],
            Vec::new(),
            vec![field(
                "snapshot_revision",
                IntentLiteralSchema::Natural,
                true,
            )],
            (0, 0),
        ),
    }
}
