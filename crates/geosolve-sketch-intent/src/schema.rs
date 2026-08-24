// SPDX-License-Identifier: GPL-3.0-or-later

//! Closed input and definition-field schemas for projectional declarations.
//!
//! These tables describe structure only. They contain no residual, geometric
//! construction, tolerance, or solver policy.

use serde::{Deserialize, Serialize};

use crate::model::{MAX_INTENT_NODE_CHILDREN, MAX_INTENT_NODE_FIELDS, MAX_INTENT_NODE_INPUTS};
use crate::{
    AggregateKind, ComputedFeatureKind, ConstraintKind, DimensionKind, ExternalIntentKind,
    GeometryRecipeKind, InputRole, InputSlot, IntentChildSchema, IntentFieldKey,
    IntentIdentityFlow, IntentKey, IntentLiteral, IntentNativeReservationKind, IntentNode,
    IntentNodeKind, IntentPortKind, IntentPortRef, IntentPortSelector, IntentUnit, LeafField,
    OperationKind, ParameterIntentKind,
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
const COMPUTED_FILLET_FIELDS_PER_CORNER: usize = 22;
#[allow(
    clippy::cast_possible_truncation,
    reason = "the source-controlled field bound is 4,096 and the derived value fits u16"
)]
const MAX_COMPUTED_FILLET_CORNERS: u16 =
    ((MAX_INTENT_NODE_FIELDS - 1) / COMPUTED_FILLET_FIELDS_PER_CORNER) as u16;

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

/// Indexed cardinality for one typed dependency role.
///
/// Repeated relation/operation operands are contiguous. Geometry point inputs
/// retain their authored semantic slot and may be sparse when only a subset
/// aliases existing stable points.
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

/// Durable mutation category exposed by the central declaration descriptor.
///
/// Hosts use this classification to build Inspector, source and code-query
/// surfaces without inferring mutation semantics from field names. It does
/// not grant authority: every edit still travels through the closed exact-CAS
/// patch vocabulary.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentEditClassification {
    Definition,
    Instance,
    InputBinding,
    Organization,
    ReadOnly,
}

/// Exact omission behavior for one definition field.
///
/// `Contextual` is deliberately explicit. It means the owning declaration
/// projection derives the omitted value from other typed state; consumers must
/// not invent a UI default. Static defaults are represented as ordinary typed
/// literals and can be shared by Inspector and code clients. `Conditional`
/// means another typed field or operand branch decides whether this field is
/// required, forbidden or inapplicable, so omission cannot be presented as a
/// default.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "default", content = "value", rename_all = "snake_case")]
pub enum IntentFieldDefault {
    Required,
    Literal(IntentLiteral),
    Contextual,
    Conditional,
}

/// Choice metadata for one definition field.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "choices", content = "values", rename_all = "snake_case")]
pub enum IntentFieldChoices {
    NotApplicable,
    Closed(Vec<IntentKey>),
    Contextual,
}

/// One definition coordinate in the central Rust declaration descriptor.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentDefinitionFieldDescriptor {
    pub schema: IntentDefinitionFieldSchema,
    pub default: IntentFieldDefault,
    pub choices: IntentFieldChoices,
    pub edit: IntentEditClassification,
}

/// One currently materialized stable output in a declaration descriptor.
///
/// The output list is generated by the same closed table which allocates the
/// graph's ports. Native reservation metadata is copied from the authenticated
/// declaration rather than restated by a presentation layer.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentOutputDescriptor {
    pub port: IntentPortRef,
    pub selector: IntentPortSelector,
    pub kind: IntentPortKind,
    pub writable: Vec<LeafField>,
    pub flow: IntentIdentityFlow,
    pub native: Option<IntentNativeReservationKind>,
    pub edit: IntentEditClassification,
}

/// Complete schema and output metadata for one concrete declaration.
///
/// This is the shared, equation-free contract consumed by GUI projections,
/// RPC snapshots and future code tooling. It intentionally contains neither
/// geometry formulas nor solver policy.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentDeclarationDescriptor {
    pub schema: IntentNodeSchema,
    pub fields: Vec<IntentDefinitionFieldDescriptor>,
    pub outputs: Vec<IntentOutputDescriptor>,
    pub suppression_edit: IntentEditClassification,
    pub input_edit: IntentEditClassification,
    pub name_edit: IntentEditClassification,
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

    /// Returns definition metadata derived from the same central schema as
    /// graph validation and concrete declaration descriptors.
    #[must_use]
    pub fn field_descriptors(&self, dynamic_children: u16) -> Vec<IntentDefinitionFieldDescriptor> {
        self.schema(dynamic_children)
            .fields
            .into_iter()
            .map(|schema| IntentDefinitionFieldDescriptor {
                default: definition_field_default(self, &schema, dynamic_children),
                choices: definition_field_choices(self, &schema),
                schema,
                edit: IntentEditClassification::Definition,
            })
            .collect()
    }
}

impl IntentNode {
    /// Returns the central schema/edit/output descriptor for this exact
    /// declaration instance.
    ///
    /// # Panics
    ///
    /// Panics only if this previously validated declaration exceeds the
    /// closed child-count bound.
    #[must_use]
    pub fn descriptor(&self) -> IntentDeclarationDescriptor {
        let dynamic_children = u16::try_from(self.child_order.len())
            .expect("validated declaration child count fits u16");
        let schema = self.kind.schema(dynamic_children);
        let fields = self.kind.field_descriptors(dynamic_children);
        let outputs = self
            .ports
            .values()
            .map(|port| {
                let native = match port.flow {
                    IntentIdentityFlow::Created { reservation } => self
                        .reservations
                        .get(&reservation)
                        .map(|reservation| reservation.kind),
                    IntentIdentityFlow::OwnedLogical
                    | IntentIdentityFlow::Aliased { .. }
                    | IntentIdentityFlow::Continued { .. }
                    | IntentIdentityFlow::Retired { .. } => None,
                };
                IntentOutputDescriptor {
                    port: port.as_ref(self.id),
                    selector: port.selector,
                    kind: port.kind,
                    writable: port.writable.clone(),
                    flow: port.flow,
                    native,
                    edit: if port.writable.is_empty() {
                        IntentEditClassification::ReadOnly
                    } else {
                        IntentEditClassification::Instance
                    },
                }
            })
            .collect();
        IntentDeclarationDescriptor {
            schema,
            fields,
            outputs,
            suppression_edit: IntentEditClassification::Definition,
            input_edit: IntentEditClassification::InputBinding,
            name_edit: IntentEditClassification::Organization,
        }
    }
}

fn definition_field_default(
    kind: &IntentNodeKind,
    schema: &IntentDefinitionFieldSchema,
    dynamic_children: u16,
) -> IntentFieldDefault {
    if schema.required {
        return IntentFieldDefault::Required;
    }
    let name = schema.field.0.as_str();
    let literal = match kind {
        IntentNodeKind::Geometry { recipe } => {
            geometry_field_default(*recipe, name, dynamic_children)
        }
        IntentNodeKind::Constraint { constraint } => constraint_field_default(*constraint, name),
        IntentNodeKind::Dimension { dimension } => dimension_field_default(*dimension, name),
        IntentNodeKind::Operation { .. }
        | IntentNodeKind::ComputedFeature { .. }
        | IntentNodeKind::Aggregate { .. }
        | IntentNodeKind::Parameter { .. }
        | IntentNodeKind::External { .. }
        | IntentNodeKind::Bootstrap { .. }
        | IntentNodeKind::Annotation
        | IntentNodeKind::Identity { .. } => None,
    };
    if let Some(literal) = literal {
        return IntentFieldDefault::Literal(literal);
    }
    if conditional_field_default(kind, name) {
        return IntentFieldDefault::Conditional;
    }
    IntentFieldDefault::Contextual
}

fn geometry_field_default(
    recipe: GeometryRecipeKind,
    name: &str,
    dynamic_children: u16,
) -> Option<IntentLiteral> {
    match (recipe, name) {
        (_, "role") => Some(enum_literal("profile")),
        (_, "regularized" | "closed") => Some(IntentLiteral::Boolean(false)),
        (
            GeometryRecipeKind::CenterArc
            | GeometryRecipeKind::ThreePointArc
            | GeometryRecipeKind::TangentArc
            | GeometryRecipeKind::CenterAxesEllipticalArc
            | GeometryRecipeKind::AxisEndpointsEllipticalArc,
            "sweep",
        ) => Some(enum_literal("counter_clockwise")),
        (GeometryRecipeKind::Hyperbola, "branch") => Some(enum_literal("positive")),
        (
            GeometryRecipeKind::OpenControlNurbs | GeometryRecipeKind::PeriodicControlNurbs,
            "degree",
        ) if dynamic_children > 3 => Some(IntentLiteral::Natural(3)),
        (
            GeometryRecipeKind::OpenControlNurbs | GeometryRecipeKind::PeriodicControlNurbs,
            "gauge_index",
        ) => Some(IntentLiteral::Natural(0)),
        (GeometryRecipeKind::RationalQuadraticConic, "weighted_middle") => {
            Some(IntentLiteral::Point([1.0, 1.0]))
        }
        (
            GeometryRecipeKind::TangentArc,
            "source_parameter" | "source_domain_upper" | "source_neighborhood_upper",
        ) => Some(quantity_literal(1.0, IntentUnit::Dimensionless)),
        (GeometryRecipeKind::TangentArc, "source_winding") => Some(IntentLiteral::Integer(0)),
        (GeometryRecipeKind::TangentArc, "source_domain") => Some(enum_literal("bounded")),
        (GeometryRecipeKind::TangentArc, "source_domain_lower" | "source_neighborhood_lower") => {
            Some(quantity_literal(0.0, IntentUnit::Dimensionless))
        }
        (GeometryRecipeKind::TangentArc, "source_domain_period") => Some(quantity_literal(
            std::f64::consts::TAU,
            IntentUnit::Dimensionless,
        )),
        (GeometryRecipeKind::TangentArc, "source_neighborhood") => Some(enum_literal("end")),
        (GeometryRecipeKind::TangentArc, "orientation") => Some(enum_literal("aligned")),
        _ => None,
    }
}

fn constraint_field_default(constraint: ConstraintKind, name: &str) -> Option<IntentLiteral> {
    match (constraint, name) {
        (_, "axis") => Some(enum_literal("x")),
        (_, "direction" | "first_direction" | "second_direction") => Some(enum_literal("forward")),
        (ConstraintKind::FixedCoordinate, "target") => {
            Some(quantity_literal(0.0, IntentUnit::Length))
        }
        (ConstraintKind::CircleCircleTangency, "mode") => Some(enum_literal("external")),
        (ConstraintKind::CircleCircleTangency, "center_direction") => {
            Some(IntentLiteral::Point([1.0, 0.0]))
        }
        (ConstraintKind::LineCircleTangency | ConstraintKind::CurveDirection, "side") => {
            Some(enum_literal("left"))
        }
        (ConstraintKind::CircleArcTangency, "side") => Some(enum_literal("outside_arc")),
        (ConstraintKind::CurveDirection, "relation") => Some(enum_literal("tangent")),
        (ConstraintKind::CurveDirection, "orientation") => Some(enum_literal("aligned")),
        (ConstraintKind::EqualCurvature, "relation") => Some(enum_literal("signed")),
        (ConstraintKind::EndpointContinuity, "continuity") => Some(enum_literal("g0")),
        (ConstraintKind::EndpointContinuity, "parameter_ratio") => {
            Some(quantity_literal(1.0, IntentUnit::Dimensionless))
        }
        _ => constraint_contact_field_default(constraint, name),
    }
}

fn dimension_field_default(dimension: DimensionKind, name: &str) -> Option<IntentLiteral> {
    match (dimension, name) {
        (_, "mode") => Some(enum_literal("driving")),
        (DimensionKind::OrientedAngle, "orientation") => Some(enum_literal("counter_clockwise")),
        (
            DimensionKind::SupportingLineOffset | DimensionKind::ExactTranslatedSegmentOffset,
            "side",
        ) => Some(enum_literal("left")),
        (
            DimensionKind::SupportingLineOffset | DimensionKind::ExactTranslatedSegmentOffset,
            "orientation",
        ) => Some(enum_literal("same")),
        _ => None,
    }
}

#[derive(Clone, Copy)]
struct ContactFieldDefaults {
    parameter: f64,
    domain: &'static str,
    neighborhood: &'static str,
    orientation: &'static str,
}

fn constraint_contact_field_default(
    constraint: ConstraintKind,
    name: &str,
) -> Option<IntentLiteral> {
    let (prefix, suffix) = ["first_contact", "second_contact", "contact"]
        .into_iter()
        .find_map(|prefix| {
            name.strip_prefix(prefix)
                .and_then(|suffix| suffix.strip_prefix('_'))
                .map(|suffix| (prefix, suffix))
        })?;
    let defaults = constraint_contact_defaults(constraint, prefix)?;
    match suffix {
        "parameter" => Some(quantity_literal(
            defaults.parameter,
            IntentUnit::Dimensionless,
        )),
        "winding" => Some(IntentLiteral::Integer(0)),
        "domain" => Some(enum_literal(defaults.domain)),
        "domain_lower" | "neighborhood_lower" => {
            Some(quantity_literal(0.0, IntentUnit::Dimensionless))
        }
        "domain_upper" | "neighborhood_upper" => {
            Some(quantity_literal(1.0, IntentUnit::Dimensionless))
        }
        "domain_period" => Some(quantity_literal(
            std::f64::consts::TAU,
            IntentUnit::Dimensionless,
        )),
        "neighborhood" => Some(enum_literal(defaults.neighborhood)),
        "orientation" => Some(enum_literal(defaults.orientation)),
        _ => None,
    }
}

fn constraint_contact_defaults(
    constraint: ConstraintKind,
    prefix: &str,
) -> Option<ContactFieldDefaults> {
    use ConstraintKind as C;

    let bounded = |parameter, neighborhood, orientation| ContactFieldDefaults {
        parameter,
        domain: "bounded",
        neighborhood,
        orientation,
    };
    let periodic = |parameter, orientation| ContactFieldDefaults {
        parameter,
        domain: "periodic",
        neighborhood: "interior",
        orientation,
    };
    match (constraint, prefix) {
        (C::LineCurveTangency, "contact") => Some(bounded(0.0, "start", "aligned")),
        (C::LineCircleTangency, "first_contact")
        | (C::CircleArcTangency, "second_contact")
        | (C::CurveCurveTangency, "first_contact" | "second_contact") => {
            Some(bounded(0.5, "interior", "aligned"))
        }
        (C::LineCircleTangency, "second_contact") | (C::CircleArcTangency, "first_contact") => {
            Some(periodic(0.0, "aligned"))
        }
        (C::PointOnCurve | C::CurveDirection, "contact")
        | (
            C::CurveCurveContact | C::EqualCurvature | C::LineLineFillet | C::CurveCurveFillet,
            "first_contact" | "second_contact",
        ) => Some(bounded(0.5, "interior", "none")),
        (C::EndpointContinuity, "first_contact") => Some(bounded(1.0, "end", "none")),
        (C::EndpointContinuity, "second_contact") => Some(bounded(0.0, "start", "none")),
        _ => None,
    }
}

fn conditional_field_default(kind: &IntentNodeKind, name: &str) -> bool {
    match kind {
        IntentNodeKind::Dimension {
            dimension: DimensionKind::ProfileOffset,
        } => matches!(name, "direction" | "side"),
        IntentNodeKind::Operation {
            operation: OperationKind::ProfileOffset,
        } => matches!(name, "direction" | "side" | "first_traversal"),
        IntentNodeKind::Operation {
            operation: OperationKind::AssociativeFillet,
        }
        | IntentNodeKind::ComputedFeature {
            feature: ComputedFeatureKind::FilletSet,
        } => {
            name.ends_with("_local_lower")
                || name.ends_with("_local_upper")
                || name.ends_with("_anchor_parameter")
                || name.ends_with("_anchor_winding")
        }
        IntentNodeKind::External {
            external: ExternalIntentKind::Binding,
        } => name == "topology_digest",
        IntentNodeKind::Geometry { .. }
        | IntentNodeKind::Constraint { .. }
        | IntentNodeKind::Dimension { .. }
        | IntentNodeKind::Operation { .. }
        | IntentNodeKind::Aggregate { .. }
        | IntentNodeKind::Parameter { .. }
        | IntentNodeKind::External { .. }
        | IntentNodeKind::Bootstrap { .. }
        | IntentNodeKind::Annotation
        | IntentNodeKind::Identity { .. } => false,
    }
}

fn definition_field_choices(
    kind: &IntentNodeKind,
    schema: &IntentDefinitionFieldSchema,
) -> IntentFieldChoices {
    if schema.literal != IntentLiteralSchema::Enum {
        return IntentFieldChoices::NotApplicable;
    }
    let name = schema.field.0.as_str();
    let values = match kind {
        IntentNodeKind::Geometry { recipe } => geometry_field_choices(*recipe, name),
        IntentNodeKind::Constraint { constraint } => constraint_field_choices(*constraint, name),
        IntentNodeKind::Dimension { dimension } => dimension_field_choices(*dimension, name),
        IntentNodeKind::Operation { operation } => operation_field_choices(*operation, name),
        IntentNodeKind::ComputedFeature { feature } => {
            computed_feature_field_choices(*feature, name)
        }
        IntentNodeKind::Parameter { parameter } => parameter_field_choices(*parameter, name),
        IntentNodeKind::External { external } => external_field_choices(*external, name),
        IntentNodeKind::Aggregate { .. }
        | IntentNodeKind::Bootstrap { .. }
        | IntentNodeKind::Annotation
        | IntentNodeKind::Identity { .. } => None,
    };
    let Some(values) = values else {
        return IntentFieldChoices::Contextual;
    };
    IntentFieldChoices::Closed(
        values
            .iter()
            .map(|value| {
                IntentKey::new(*value).expect("source-controlled intent enum choice is valid")
            })
            .collect(),
    )
}

fn geometry_field_choices(
    recipe: GeometryRecipeKind,
    name: &str,
) -> Option<&'static [&'static str]> {
    match (recipe, name) {
        (_, "role") => Some(&["profile", "construction"]),
        (_, "sweep") => Some(&["counter_clockwise", "clockwise"]),
        (GeometryRecipeKind::Hyperbola, "branch") => Some(&["positive", "negative"]),
        (GeometryRecipeKind::TangentArc, "orientation") => Some(&["aligned", "opposed"]),
        (GeometryRecipeKind::TangentArc, "source_domain") => {
            Some(&["bounded", "supporting_line", "periodic"])
        }
        (GeometryRecipeKind::TangentArc, "source_neighborhood") => {
            Some(&["start", "end", "interior", "local"])
        }
        _ => None,
    }
}

fn constraint_field_choices(
    constraint: ConstraintKind,
    name: &str,
) -> Option<&'static [&'static str]> {
    match (constraint, name) {
        (_, "axis") => Some(&["x", "y"]),
        (_, "direction" | "first_direction" | "second_direction") => Some(&["forward", "reverse"]),
        (ConstraintKind::LineCircleTangency | ConstraintKind::CurveDirection, "side")
        | (
            ConstraintKind::LineLineFillet | ConstraintKind::CurveCurveFillet,
            "first_side" | "second_side",
        ) => Some(&["left", "right"]),
        (ConstraintKind::CircleCircleTangency, "mode") => {
            Some(&["external", "first_contains_second", "second_contains_first"])
        }
        (ConstraintKind::CircleArcTangency, "side") => Some(&["outside_arc", "inside_arc"]),
        (ConstraintKind::LineCurveTangency, "endpoint") => Some(&["start", "end"]),
        (ConstraintKind::CurveDirection, "relation") => Some(&["tangent", "normal"]),
        (ConstraintKind::CurveDirection, "orientation") => Some(&["aligned", "opposed"]),
        (ConstraintKind::EqualCurvature, "relation") => {
            Some(&["signed", "magnitude_same_sign", "magnitude_opposite_sign"])
        }
        (ConstraintKind::EndpointContinuity, "continuity") => {
            Some(&["g0", "g1", "g2", "parametric_c2"])
        }
        (ConstraintKind::LineLineFillet | ConstraintKind::CurveCurveFillet, "endpoint_order") => {
            Some(&["first_then_second", "second_then_first"])
        }
        (ConstraintKind::CurveCurveFillet, "first_trim_endpoint" | "second_trim_endpoint") => {
            Some(&["start", "end"])
        }
        (_, name) if name.ends_with("_domain") => Some(&["supporting_line", "bounded", "periodic"]),
        (_, name) if name.ends_with("_neighborhood") => {
            Some(&["interior", "start", "end", "local"])
        }
        (_, name) if name.ends_with("_orientation") => {
            Some(&["none", "unoriented", "aligned", "opposed"])
        }
        _ => None,
    }
}

fn dimension_field_choices(
    dimension: DimensionKind,
    name: &str,
) -> Option<&'static [&'static str]> {
    match (dimension, name) {
        (DimensionKind::ProfileOffset, "mode") => Some(&["driving"]),
        (_, "mode") => Some(&["driving", "reference"]),
        (DimensionKind::OrientedAngle, "orientation") => Some(&["counter_clockwise", "clockwise"]),
        (
            DimensionKind::SupportingLineOffset
            | DimensionKind::ExactTranslatedSegmentOffset
            | DimensionKind::ProfileOffset,
            "side",
        ) => Some(&["left", "right"]),
        (
            DimensionKind::SupportingLineOffset | DimensionKind::ExactTranslatedSegmentOffset,
            "orientation",
        ) => Some(&["same", "reversed"]),
        (DimensionKind::ProfileOffset, "direction") => Some(&["outward", "inward"]),
        (DimensionKind::ProfileOffset, "source_traversal" | "target_traversal") => {
            Some(&["forward", "reverse"])
        }
        _ => None,
    }
}

fn operation_field_choices(
    operation: OperationKind,
    name: &str,
) -> Option<&'static [&'static str]> {
    use OperationKind as O;

    match (operation, name) {
        (O::Split | O::Break | O::Trim, "retained") => Some(&["before", "after"]),
        (O::Extend, "endpoint") => Some(&["start", "end"]),
        (O::AssociativeFillet, "radius_mode") => Some(&["driving", "reference"]),
        (O::AssociativeFillet, "endpoint_order") => {
            Some(&["first_then_second", "second_then_first"])
        }
        (O::AssociativeFillet, "sweep") => Some(&["counter_clockwise", "clockwise"]),
        (O::AssociativeFillet, "first_neighborhood" | "second_neighborhood") => {
            Some(&["interior", "local", "start", "end"])
        }
        (O::AssociativeFillet, "first_normal_side" | "second_normal_side") => {
            Some(&["left", "right"])
        }
        (O::AssociativeFillet, "first_trim_endpoint" | "second_trim_endpoint") => {
            Some(&["start", "end"])
        }
        (O::Rectangle | O::RegularPolygon | O::Slot, "role") => Some(&["profile", "construction"]),
        (O::ProfileOffset, "direction") => Some(&["outward", "inward"]),
        (O::ProfileOffset, "side") => Some(&["left", "right"]),
        (O::ProfileOffset, "first_traversal") => Some(&["forward", "reverse"]),
        _ => None,
    }
}

fn computed_feature_field_choices(
    feature: ComputedFeatureKind,
    name: &str,
) -> Option<&'static [&'static str]> {
    match feature {
        ComputedFeatureKind::FilletSet if name.ends_with("_neighborhood") => {
            Some(&["interior", "local", "start", "end"])
        }
        ComputedFeatureKind::FilletSet if name.ends_with("_normal_side") => {
            Some(&["left", "right"])
        }
        ComputedFeatureKind::FilletSet if name.ends_with("_trim_endpoint") => {
            Some(&["start", "end"])
        }
        ComputedFeatureKind::FilletSet if name.ends_with("_endpoint_order") => {
            Some(&["first_then_second", "second_then_first"])
        }
        ComputedFeatureKind::FilletSet if name.ends_with("_sweep") => {
            Some(&["counter_clockwise", "clockwise"])
        }
        ComputedFeatureKind::FilletSet => None,
    }
}

fn parameter_field_choices(
    parameter: ParameterIntentKind,
    name: &str,
) -> Option<&'static [&'static str]> {
    match (parameter, name) {
        (ParameterIntentKind::Parameter, "kind") => {
            Some(&["length", "angle", "dimensionless", "activation"])
        }
        _ => None,
    }
}

fn external_field_choices(
    external: ExternalIntentKind,
    name: &str,
) -> Option<&'static [&'static str]> {
    match (external, name) {
        (ExternalIntentKind::Binding, "feature_kind") => Some(&["point", "line_segment"]),
        _ => None,
    }
}

fn enum_literal(value: &str) -> IntentLiteral {
    IntentLiteral::Enum(
        IntentKey::new(value).expect("source-controlled intent enum default is valid"),
    )
}

const fn quantity_literal(value: f64, unit: IntentUnit) -> IntentLiteral {
    IntentLiteral::Quantity { value, unit }
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

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive table keeps every geometry recipe's structural fields and cardinality reviewable"
)]
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
        | G::CenterRectangle
        | G::ThreePointCenterRectangle
        | G::RationalQuadraticConic => 2,
        G::ThreePointCornerRectangle
        | G::ThreePointCircle
        | G::CenterArc
        | G::ThreePointArc
        | G::CenterAxesEllipse
        | G::AxisEndpointsEllipse
        | G::QuadraticBezier => 3,
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
            // Cubic is the ordinary fallback, but two- and three-control
            // declarations cannot admit it. Require an explicit admissible
            // degree for those exact declaration instances.
            field(
                "degree",
                IntentLiteralSchema::Natural,
                dynamic_children <= 3,
            ),
            field("gauge_index", IntentLiteralSchema::Natural, false),
        ]);
    }
    // Every geometry recipe may declare its persistent sketch role. Omitting
    // the field preserves the ordinary profile default; the materializer owns
    // the closed `profile` / `construction` interpretation.
    fields.push(field("role", IntentLiteralSchema::Enum, false));
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
    let contact_count = match kind {
        C::PointOnCurve | C::LineCurveTangency | C::CurveDirection => 1,
        C::LineCircleTangency
        | C::CircleArcTangency
        | C::CurveCurveContact
        | C::CurveCurveTangency
        | C::EqualCurvature
        | C::EndpointContinuity
        | C::LineLineFillet
        | C::CurveCurveFillet => 2,
        _ => 0,
    };
    for index in 0..contact_count {
        let prefix = match (contact_count, index) {
            (1, _) => "contact",
            (_, 0) => "first_contact",
            _ => "second_contact",
        };
        fields.extend([
            field(
                &format!("{prefix}_parameter"),
                IntentLiteralSchema::Quantity(IntentUnit::Dimensionless),
                false,
            ),
            field(
                &format!("{prefix}_winding"),
                IntentLiteralSchema::Integer,
                false,
            ),
            field(
                &format!("{prefix}_domain"),
                IntentLiteralSchema::Enum,
                false,
            ),
            field(
                &format!("{prefix}_domain_lower"),
                IntentLiteralSchema::Quantity(IntentUnit::Dimensionless),
                false,
            ),
            field(
                &format!("{prefix}_domain_upper"),
                IntentLiteralSchema::Quantity(IntentUnit::Dimensionless),
                false,
            ),
            field(
                &format!("{prefix}_domain_period"),
                IntentLiteralSchema::Quantity(IntentUnit::Dimensionless),
                false,
            ),
            field(
                &format!("{prefix}_neighborhood"),
                IntentLiteralSchema::Enum,
                false,
            ),
            field(
                &format!("{prefix}_neighborhood_lower"),
                IntentLiteralSchema::Quantity(IntentUnit::Dimensionless),
                false,
            ),
            field(
                &format!("{prefix}_neighborhood_upper"),
                IntentLiteralSchema::Quantity(IntentUnit::Dimensionless),
                false,
            ),
            field(
                &format!("{prefix}_orientation"),
                IntentLiteralSchema::Enum,
                false,
            ),
        ]);
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
            // The target support is explicit. The current declaration can
            // reconstruct one-junction-free edge pair exactly; richer topology
            // remains fail-closed until its owner/branch schema is present.
            input(InputRole::Span, 0, 1),
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
            field("source_traversal", IntentLiteralSchema::Enum, true),
            field("target_traversal", IntentLiteralSchema::Enum, true),
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
            vec![
                field(
                    "radius",
                    IntentLiteralSchema::Quantity(IntentUnit::Length),
                    true,
                ),
                field("radius_mode", IntentLiteralSchema::Enum, true),
                field("endpoint_order", IntentLiteralSchema::Enum, true),
                field("sweep", IntentLiteralSchema::Enum, true),
                field(
                    "first_parameter",
                    IntentLiteralSchema::Quantity(IntentUnit::Dimensionless),
                    true,
                ),
                field("first_winding", IntentLiteralSchema::Integer, true),
                field("first_neighborhood", IntentLiteralSchema::Enum, true),
                field(
                    "first_local_lower",
                    IntentLiteralSchema::Quantity(IntentUnit::Dimensionless),
                    false,
                ),
                field(
                    "first_local_upper",
                    IntentLiteralSchema::Quantity(IntentUnit::Dimensionless),
                    false,
                ),
                field("first_normal_side", IntentLiteralSchema::Enum, true),
                field("first_trim_endpoint", IntentLiteralSchema::Enum, true),
                field("first_periodic_anchor", IntentLiteralSchema::Boolean, true),
                field(
                    "first_anchor_parameter",
                    IntentLiteralSchema::Quantity(IntentUnit::Dimensionless),
                    false,
                ),
                field("first_anchor_winding", IntentLiteralSchema::Integer, false),
                field(
                    "second_parameter",
                    IntentLiteralSchema::Quantity(IntentUnit::Dimensionless),
                    true,
                ),
                field("second_winding", IntentLiteralSchema::Integer, true),
                field("second_neighborhood", IntentLiteralSchema::Enum, true),
                field(
                    "second_local_lower",
                    IntentLiteralSchema::Quantity(IntentUnit::Dimensionless),
                    false,
                ),
                field(
                    "second_local_upper",
                    IntentLiteralSchema::Quantity(IntentUnit::Dimensionless),
                    false,
                ),
                field("second_normal_side", IntentLiteralSchema::Enum, true),
                field("second_trim_endpoint", IntentLiteralSchema::Enum, true),
                field("second_periodic_anchor", IntentLiteralSchema::Boolean, true),
                field(
                    "second_anchor_parameter",
                    IntentLiteralSchema::Quantity(IntentUnit::Dimensionless),
                    false,
                ),
                field("second_anchor_winding", IntentLiteralSchema::Integer, false),
            ],
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
                field("role", IntentLiteralSchema::Enum, true),
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
                field("role", IntentLiteralSchema::Enum, true),
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
                field("role", IntentLiteralSchema::Enum, true),
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
                input(InputRole::Profile, 0, MAX_SCHEMA_INPUTS),
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
                field("first_traversal", IntentLiteralSchema::Enum, false),
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
        ComputedFeatureKind::FilletSet => {
            let mut fields = vec![field(
                "radius",
                IntentLiteralSchema::Quantity(IntentUnit::Length),
                true,
            )];
            for corner in 0..dynamic_children {
                for parent in ["first", "second"] {
                    let prefix = format!("corner_{corner:04}_{parent}");
                    fields.extend([
                        field(
                            &format!("{prefix}_parameter"),
                            IntentLiteralSchema::Quantity(IntentUnit::Dimensionless),
                            true,
                        ),
                        field(
                            &format!("{prefix}_winding"),
                            IntentLiteralSchema::Integer,
                            true,
                        ),
                        field(
                            &format!("{prefix}_neighborhood"),
                            IntentLiteralSchema::Enum,
                            true,
                        ),
                        field(
                            &format!("{prefix}_local_lower"),
                            IntentLiteralSchema::Quantity(IntentUnit::Dimensionless),
                            false,
                        ),
                        field(
                            &format!("{prefix}_local_upper"),
                            IntentLiteralSchema::Quantity(IntentUnit::Dimensionless),
                            false,
                        ),
                        field(
                            &format!("{prefix}_normal_side"),
                            IntentLiteralSchema::Enum,
                            true,
                        ),
                        field(
                            &format!("{prefix}_trim_endpoint"),
                            IntentLiteralSchema::Enum,
                            true,
                        ),
                        field(
                            &format!("{prefix}_periodic_anchor"),
                            IntentLiteralSchema::Boolean,
                            true,
                        ),
                        field(
                            &format!("{prefix}_anchor_parameter"),
                            IntentLiteralSchema::Quantity(IntentUnit::Dimensionless),
                            false,
                        ),
                        field(
                            &format!("{prefix}_anchor_winding"),
                            IntentLiteralSchema::Integer,
                            false,
                        ),
                    ]);
                }
                fields.extend([
                    field(
                        &format!("corner_{corner:04}_endpoint_order"),
                        IntentLiteralSchema::Enum,
                        true,
                    ),
                    field(
                        &format!("corner_{corner:04}_sweep"),
                        IntentLiteralSchema::Enum,
                        true,
                    ),
                ]);
            }
            schema(
                vec![input(
                    InputRole::Span,
                    dynamic_children.saturating_mul(2),
                    dynamic_children.saturating_mul(2),
                )],
                Vec::new(),
                fields,
                (1, MAX_COMPUTED_FILLET_CORNERS),
            )
        }
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
                input(InputRole::Point, 0, 1),
                input(InputRole::Contact, 0, 1),
                input(InputRole::Curve, 0, 1),
                input(InputRole::Dimension, 0, 1),
                input(InputRole::Scalar, 0, 1),
                input(InputRole::Constraint, 0, 1),
                input(InputRole::External, 0, 1),
                input(InputRole::Source, 0, 1),
            ],
            vec![choice(
                &[
                    (InputRole::Point, 0),
                    (InputRole::Contact, 0),
                    (InputRole::Curve, 0),
                    (InputRole::Dimension, 0),
                    (InputRole::Scalar, 0),
                    (InputRole::Constraint, 0),
                    (InputRole::External, 0),
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
            vec![
                field("feature_kind", IntentLiteralSchema::Enum, true),
                field("topology_digest", IntentLiteralSchema::Text, false),
            ],
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
