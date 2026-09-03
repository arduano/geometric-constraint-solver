// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use geosolve_constraint_editor::ProjectionalEditorSession;
use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_code::{
    CODE_AUTHORING_FAMILIES, CodeAuthoringAvailability, CodeAuthoringDeclarationKind, CodeProject,
    CompiledManagedSource, EditorBootstrapDeclaration, KeyedReconcileState, ManagedPathSegment,
    ManagedValue, ProjectKey, SemanticSymbol, materialize_code_project_cold,
    prepare_editor_declaration_insertions, required_generated_members,
    resolve_code_authoring_declaration,
};
use geosolve_sketch_intent::{
    ConstraintKind, GeometryRecipeKind, InputRole, InputSlot, IntentFieldKey, IntentKey,
    IntentLiteral, IntentNode, IntentNodeDraft, IntentNodeKind, IntentPatch, IntentPatchOperation,
    IntentPatchPolicy, IntentPortKind, IntentPortRole, IntentPortSelector, IntentProjectionPath,
    IntentProjectionPathSegment, IntentSessionId, IntentUnit, LeafField, PatchPortRef,
};

const EMPTY_MANAGED: &str =
    include_str!("../../../packages/geosolve-sketch-code/test/fixtures/managed-clean-empty.json");

#[derive(Clone)]
struct NamedDraft {
    alias: &'static str,
    draft: IntentNodeDraft,
}

struct ReplayFixture {
    geometry: Vec<NamedDraft>,
    relation: IntentNodeDraft,
}

enum CoverageCase {
    Replay(ReplayFixture),
    /// Managed source deliberately has no declaration for host-owned
    /// external bindings/snapshots yet. These two rows therefore prove the
    /// same closed kind/input/field/output wire schema without inventing a
    /// source construct which could not honestly cold replay.
    ExternalWireOnly(IntentNodeDraft),
}

fn key(value: &str) -> IntentKey {
    IntentKey::new(value).expect("fixture key")
}

const fn selector(role: IntentPortRole) -> IntentPortSelector {
    IntentPortSelector::Node { role, index: 0 }
}

const fn indexed_selector(role: IntentPortRole, index: u16) -> IntentPortSelector {
    IntentPortSelector::Node { role, index }
}

fn alias(node: &str, role: IntentPortRole) -> PatchPortRef {
    PatchPortRef::Alias {
        node: key(node),
        selector: selector(role),
    }
}

const fn length(value: f64) -> IntentLiteral {
    IntentLiteral::Quantity {
        value,
        unit: IntentUnit::Length,
    }
}

const fn parameter(value: f64) -> IntentLiteral {
    IntentLiteral::Quantity {
        value,
        unit: IntentUnit::Dimensionless,
    }
}

const fn angle(value: f64) -> IntentLiteral {
    IntentLiteral::Quantity {
        value,
        unit: IntentUnit::Angle,
    }
}

fn geometry(alias: &'static str, draft: IntentNodeDraft) -> NamedDraft {
    NamedDraft { alias, draft }
}

fn point(name: &str, position: [f64; 2]) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::SketchPoint,
        },
        key(name),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Primary),
        LeafField::X,
        length(position[0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Primary),
        LeafField::Y,
        length(position[1]),
    )
}

fn segment(name: &str, start: [f64; 2], end: [f64; 2]) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        },
        key(name),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Start),
        LeafField::X,
        length(start[0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Start),
        LeafField::Y,
        length(start[1]),
    )
    .with_instance_leaf(selector(IntentPortRole::End), LeafField::X, length(end[0]))
    .with_instance_leaf(selector(IntentPortRole::End), LeafField::Y, length(end[1]))
}

fn circle(name: &str, center: [f64; 2], radius: f64) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::CenterRadiusCircle,
        },
        key(name),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Center),
        LeafField::X,
        length(center[0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Center),
        LeafField::Y,
        length(center[1]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Target),
        LeafField::Value,
        length(radius),
    )
}

fn circular_arc(
    name: &str,
    center: [f64; 2],
    radius: f64,
    start: f64,
    end: f64,
) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::CenterArc,
        },
        key(name),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Center),
        LeafField::X,
        length(center[0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Center),
        LeafField::Y,
        length(center[1]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Target),
        LeafField::Value,
        length(radius),
    )
    .with_instance_leaf(
        indexed_selector(IntentPortRole::Target, 1),
        LeafField::Angle,
        angle(start),
    )
    .with_instance_leaf(
        indexed_selector(IntentPortRole::Target, 2),
        LeafField::Angle,
        angle(end),
    )
}

fn quadratic_bezier(
    name: &str,
    start: [f64; 2],
    control: [f64; 2],
    end: [f64; 2],
) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::QuadraticBezier,
        },
        key(name),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Start),
        LeafField::X,
        length(start[0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Start),
        LeafField::Y,
        length(start[1]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Control),
        LeafField::X,
        length(control[0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Control),
        LeafField::Y,
        length(control[1]),
    )
    .with_instance_leaf(selector(IntentPortRole::End), LeafField::X, length(end[0]))
    .with_instance_leaf(selector(IntentPortRole::End), LeafField::Y, length(end[1]))
}

fn relation(
    kind: ConstraintKind,
    inputs: impl IntoIterator<Item = (InputSlot, PatchPortRef)>,
    fields: impl IntoIterator<Item = (&'static str, IntentLiteral)>,
) -> IntentNodeDraft {
    let mut draft = IntentNodeDraft::new(
        IntentNodeKind::Constraint { constraint: kind },
        key("relation"),
    );
    for (slot, source) in inputs {
        draft = draft.with_input(slot, source);
    }
    for (name, value) in fields {
        draft = draft.with_field(IntentFieldKey(key(name)), value);
    }
    draft
}

fn p(index: usize) -> PatchPortRef {
    alias(
        if index == 0 { "p0" } else { "p1" },
        IntentPortRole::Primary,
    )
}

fn span(index: usize) -> PatchPortRef {
    alias(
        if index == 0 { "line0" } else { "line1" },
        IntentPortRole::Span,
    )
}

fn curve(index: usize) -> PatchPortRef {
    alias(
        if index == 0 { "circle0" } else { "circle1" },
        IntentPortRole::Curve,
    )
}

fn point_pair() -> [(InputSlot, PatchPortRef); 2] {
    [
        (InputSlot::new(InputRole::Point, 0), p(0)),
        (InputSlot::new(InputRole::Point, 1), p(1)),
    ]
}

fn span_pair() -> [(InputSlot, PatchPortRef); 2] {
    [
        (InputSlot::new(InputRole::Span, 0), span(0)),
        (InputSlot::new(InputRole::Span, 1), span(1)),
    ]
}

fn circle_pair() -> [(InputSlot, PatchPortRef); 2] {
    [
        (InputSlot::new(InputRole::Curve, 0), curve(0)),
        (InputSlot::new(InputRole::Curve, 1), curve(1)),
    ]
}

fn replay(
    kind: ConstraintKind,
    geometry: Vec<NamedDraft>,
    inputs: impl IntoIterator<Item = (InputSlot, PatchPortRef)>,
    fields: impl IntoIterator<Item = (&'static str, IntentLiteral)>,
) -> CoverageCase {
    CoverageCase::Replay(ReplayFixture {
        geometry,
        relation: relation(kind, inputs, fields),
    })
}

fn point_geometry(first: [f64; 2], second: [f64; 2]) -> Vec<NamedDraft> {
    vec![
        geometry("p0", point("p0", first)),
        geometry("p1", point("p1", second)),
    ]
}

fn line_geometry(first: ([f64; 2], [f64; 2]), second: ([f64; 2], [f64; 2])) -> Vec<NamedDraft> {
    vec![
        geometry("line0", segment("line0", first.0, first.1)),
        geometry("line1", segment("line1", second.0, second.1)),
    ]
}

fn circle_geometry(first: ([f64; 2], f64), second: ([f64; 2], f64)) -> Vec<NamedDraft> {
    vec![
        geometry("circle0", circle("circle0", first.0, first.1)),
        geometry("circle1", circle("circle1", second.0, second.1)),
    ]
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive match makes every persistent constraint fixture compile-visible"
)]
fn coverage_case(kind: ConstraintKind) -> CoverageCase {
    use ConstraintKind as C;

    match kind {
        C::FixedPoint => replay(
            kind,
            point_geometry([0.0, 0.0], [2.0, 1.0]),
            [(InputSlot::new(InputRole::Point, 0), p(0))],
            [("target", IntentLiteral::Point([0.0, 0.0]))],
        ),
        C::FixedCoordinate => replay(
            kind,
            point_geometry([0.0, 0.0], [2.0, 1.0]),
            [(InputSlot::new(InputRole::Point, 0), p(0))],
            [
                ("axis", IntentLiteral::Enum(key("x"))),
                ("target", length(0.0)),
            ],
        ),
        C::CoincidentWithOrigin => replay(
            kind,
            point_geometry([0.0, 0.0], [2.0, 1.0]),
            [(InputSlot::new(InputRole::Point, 0), p(0))],
            [],
        ),
        C::PointOnDatumAxis => replay(
            kind,
            point_geometry([0.0, 1.0], [2.0, 1.0]),
            [(InputSlot::new(InputRole::Point, 0), p(0))],
            [("axis", IntentLiteral::Enum(key("y")))],
        ),
        C::Coincident => replay(
            kind,
            point_geometry([0.0, 0.0], [0.0, 0.0]),
            point_pair(),
            [],
        ),
        C::ExternalPointCoincident => CoverageCase::ExternalWireOnly(relation(
            kind,
            [
                (InputSlot::new(InputRole::Point, 0), p(0)),
                (
                    InputSlot::new(InputRole::External, 0),
                    alias("external", IntentPortRole::External),
                ),
            ],
            [],
        )),
        C::Horizontal => replay(
            kind,
            line_geometry(([-1.0, 0.0], [1.0, 0.0]), ([0.0, 2.0], [1.0, 3.0])),
            [(InputSlot::new(InputRole::Span, 0), span(0))],
            [],
        ),
        C::Vertical => replay(
            kind,
            line_geometry(([0.0, -1.0], [0.0, 1.0]), ([2.0, 0.0], [3.0, 1.0])),
            [(InputSlot::new(InputRole::Span, 0), span(0))],
            [],
        ),
        C::HorizontalPoints => replay(
            kind,
            point_geometry([0.0, 0.0], [2.0, 0.0]),
            point_pair(),
            [],
        ),
        C::VerticalPoints => replay(
            kind,
            point_geometry([0.0, 0.0], [0.0, 2.0]),
            point_pair(),
            [],
        ),
        C::HorizontalPointToMidpoint | C::VerticalPointToMidpoint | C::Midpoint => replay(
            kind,
            vec![
                geometry("p0", point("p0", [0.0, 0.0])),
                geometry("line0", segment("line0", [-1.0, 0.0], [1.0, 0.0])),
            ],
            [
                (InputSlot::new(InputRole::Point, 0), p(0)),
                (InputSlot::new(InputRole::Span, 0), span(0)),
            ],
            [],
        ),
        C::PointOnCurve => replay(
            kind,
            vec![
                geometry("p0", point("p0", [0.0, 0.0])),
                geometry("line0", segment("line0", [-1.0, 0.0], [1.0, 0.0])),
            ],
            [
                (InputSlot::new(InputRole::Point, 0), p(0)),
                (InputSlot::new(InputRole::Span, 0), span(0)),
            ],
            [
                ("contact_parameter", parameter(0.5)),
                ("contact_range_lower", parameter(0.25)),
                ("contact_range_upper", parameter(0.75)),
                ("contact_neighborhood", IntentLiteral::Enum(key("local"))),
                ("contact_neighborhood_lower", parameter(0.25)),
                ("contact_neighborhood_upper", parameter(0.75)),
                ("contact_orientation", IntentLiteral::Enum(key("none"))),
            ],
        ),
        C::Parallel => replay(
            kind,
            line_geometry(([-1.0, 0.0], [1.0, 0.0]), ([-1.0, 1.0], [1.0, 1.0])),
            span_pair(),
            [],
        ),
        C::Perpendicular => replay(
            kind,
            line_geometry(([-1.0, 0.0], [1.0, 0.0]), ([0.0, -1.0], [0.0, 1.0])),
            span_pair(),
            [],
        ),
        C::ExternalLineCollinear => CoverageCase::ExternalWireOnly(relation(
            kind,
            [
                (InputSlot::new(InputRole::Span, 0), span(0)),
                (
                    InputSlot::new(InputRole::External, 0),
                    alias("external", IntentPortRole::External),
                ),
            ],
            [("direction", IntentLiteral::Enum(key("reverse")))],
        )),
        C::CollinearWithDatumAxis => replay(
            kind,
            line_geometry(([-1.0, 0.0], [1.0, 0.0]), ([0.0, 2.0], [1.0, 3.0])),
            [(InputSlot::new(InputRole::Span, 0), span(0))],
            [
                ("axis", IntentLiteral::Enum(key("x"))),
                ("direction", IntentLiteral::Enum(key("forward"))),
            ],
        ),
        C::Concentric => replay(
            kind,
            circle_geometry(([0.0, 0.0], 1.0), ([0.0, 0.0], 2.0)),
            circle_pair(),
            [],
        ),
        C::Collinear => replay(
            kind,
            line_geometry(([-1.0, 0.0], [1.0, 0.0]), ([2.0, 0.0], [4.0, 0.0])),
            span_pair(),
            [
                ("first_direction", IntentLiteral::Enum(key("forward"))),
                ("second_direction", IntentLiteral::Enum(key("reverse"))),
            ],
        ),
        C::EqualLength => replay(
            kind,
            line_geometry(([-1.0, 0.0], [1.0, 0.0]), ([0.0, 1.0], [2.0, 1.0])),
            span_pair(),
            [],
        ),
        C::EqualRadius => replay(
            kind,
            circle_geometry(([0.0, 0.0], 1.0), ([3.0, 0.0], 1.0)),
            circle_pair(),
            [],
        ),
        C::SymmetricAboutLine => replay(
            kind,
            vec![
                geometry("p0", point("p0", [-1.0, 1.0])),
                geometry("p1", point("p1", [1.0, 1.0])),
                geometry("line0", segment("line0", [0.0, -1.0], [0.0, 2.0])),
            ],
            [
                (InputSlot::new(InputRole::Point, 0), p(0)),
                (InputSlot::new(InputRole::Point, 1), p(1)),
                (InputSlot::new(InputRole::Span, 0), span(0)),
            ],
            [],
        ),
        C::SymmetricAboutDatumAxis => replay(
            kind,
            point_geometry([-1.0, 1.0], [1.0, 1.0]),
            point_pair(),
            [("axis", IntentLiteral::Enum(key("y")))],
        ),
        C::LineCircleTangency => replay(
            kind,
            vec![
                geometry("line0", segment("line0", [-2.0, 0.0], [2.0, 0.0])),
                geometry("circle0", circle("circle0", [0.0, 1.0], 1.0)),
            ],
            [
                (InputSlot::new(InputRole::Span, 0), span(0)),
                (InputSlot::new(InputRole::Curve, 0), curve(0)),
            ],
            [
                ("side", IntentLiteral::Enum(key("left"))),
                ("first_contact_parameter", parameter(0.5)),
                (
                    "first_contact_support",
                    IntentLiteral::Enum(key("supporting_line")),
                ),
                (
                    "first_contact_orientation",
                    IntentLiteral::Enum(key("aligned")),
                ),
                (
                    "second_contact_parameter",
                    parameter(3.0 * std::f64::consts::FRAC_PI_2),
                ),
                ("second_contact_winding", IntentLiteral::Integer(2)),
                (
                    "second_contact_orientation",
                    IntentLiteral::Enum(key("aligned")),
                ),
            ],
        ),
        C::CircleCircleTangency => replay(
            kind,
            circle_geometry(([0.0, 0.0], 1.0), ([2.0, 0.0], 1.0)),
            circle_pair(),
            [
                ("mode", IntentLiteral::Enum(key("external"))),
                ("center_direction", IntentLiteral::Point([1.0, 0.0])),
            ],
        ),
        C::CircleArcTangency => replay(
            kind,
            vec![
                geometry("circle0", circle("circle0", [3.0, 0.0], 1.0)),
                geometry(
                    "circle1",
                    circular_arc(
                        "circle1",
                        [0.0, 0.0],
                        2.0,
                        -std::f64::consts::FRAC_PI_2,
                        std::f64::consts::FRAC_PI_2,
                    ),
                ),
            ],
            circle_pair(),
            [
                ("side", IntentLiteral::Enum(key("outside_arc"))),
                ("first_contact_parameter", parameter(std::f64::consts::PI)),
                (
                    "first_contact_orientation",
                    IntentLiteral::Enum(key("opposed")),
                ),
                ("second_contact_parameter", parameter(0.5)),
                (
                    "second_contact_orientation",
                    IntentLiteral::Enum(key("opposed")),
                ),
            ],
        ),
        C::LineCurveTangency => replay(
            kind,
            vec![
                geometry("line0", segment("line0", [0.0, 0.0], [1.0, 0.0])),
                geometry(
                    "line1",
                    quadratic_bezier("line1", [0.0, 0.0], [1.0, 0.0], [2.0, 1.0]),
                ),
            ],
            span_pair(),
            [
                ("endpoint", IntentLiteral::Enum(key("start"))),
                ("contact_parameter", parameter(0.0)),
                ("contact_neighborhood", IntentLiteral::Enum(key("start"))),
                ("contact_orientation", IntentLiteral::Enum(key("aligned"))),
            ],
        ),
        C::CurveCurveContact => replay(
            kind,
            line_geometry(([-1.0, 0.0], [1.0, 0.0]), ([0.0, -1.0], [0.0, 1.0])),
            span_pair(),
            [
                ("first_contact_parameter", parameter(0.5)),
                ("second_contact_parameter", parameter(0.5)),
                (
                    "first_contact_orientation",
                    IntentLiteral::Enum(key("none")),
                ),
                (
                    "second_contact_orientation",
                    IntentLiteral::Enum(key("none")),
                ),
            ],
        ),
        C::CurveCurveTangency => replay(
            kind,
            vec![
                geometry(
                    "line0",
                    quadratic_bezier("line0", [0.0, 0.0], [1.0, 0.0], [2.0, 1.0]),
                ),
                geometry(
                    "line1",
                    quadratic_bezier("line1", [0.0, 0.0], [1.0, 0.0], [2.0, -1.0]),
                ),
            ],
            span_pair(),
            [
                ("first_contact_parameter", parameter(0.0)),
                (
                    "first_contact_neighborhood",
                    IntentLiteral::Enum(key("start")),
                ),
                (
                    "first_contact_orientation",
                    IntentLiteral::Enum(key("aligned")),
                ),
                ("second_contact_parameter", parameter(0.0)),
                (
                    "second_contact_neighborhood",
                    IntentLiteral::Enum(key("start")),
                ),
                (
                    "second_contact_orientation",
                    IntentLiteral::Enum(key("aligned")),
                ),
            ],
        ),
        C::CurveDirection => replay(
            kind,
            vec![
                geometry("line0", segment("line0", [0.0, 2.0], [1.0, 2.0])),
                geometry(
                    "line1",
                    quadratic_bezier("line1", [0.0, 0.0], [1.0, 0.0], [2.0, 1.0]),
                ),
            ],
            span_pair(),
            [
                ("relation", IntentLiteral::Enum(key("tangent"))),
                ("orientation", IntentLiteral::Enum(key("aligned"))),
                ("contact_parameter", parameter(0.0)),
                ("contact_neighborhood", IntentLiteral::Enum(key("start"))),
            ],
        ),
        C::EqualCurvature => replay(
            kind,
            line_geometry(([-1.0, 0.0], [1.0, 0.0]), ([-1.0, 1.0], [1.0, 1.0])),
            span_pair(),
            [
                ("relation", IntentLiteral::Enum(key("signed"))),
                ("first_contact_parameter", parameter(0.25)),
                ("second_contact_parameter", parameter(0.75)),
            ],
        ),
        C::EndpointContinuity => replay(
            kind,
            line_geometry(([0.0, 0.0], [2.0, 0.0]), ([2.0, 0.0], [6.0, 0.0])),
            span_pair(),
            [
                ("continuity", IntentLiteral::Enum(key("parametric_c2"))),
                ("parameter_ratio", parameter(2.0)),
                ("first_contact_parameter", parameter(1.0)),
                (
                    "first_contact_neighborhood",
                    IntentLiteral::Enum(key("end")),
                ),
                ("second_contact_parameter", parameter(0.0)),
                (
                    "second_contact_neighborhood",
                    IntentLiteral::Enum(key("start")),
                ),
            ],
        ),
        C::LineLineFillet | C::CurveCurveFillet => {
            let mut fields = vec![
                ("first_side", IntentLiteral::Enum(key("left"))),
                ("second_side", IntentLiteral::Enum(key("left"))),
                (
                    "endpoint_order",
                    IntentLiteral::Enum(key("first_then_second")),
                ),
                ("first_contact_parameter", parameter(0.75)),
                (
                    "first_contact_neighborhood",
                    IntentLiteral::Enum(key("interior")),
                ),
                ("second_contact_parameter", parameter(0.25)),
                (
                    "second_contact_neighborhood",
                    IntentLiteral::Enum(key("interior")),
                ),
            ];
            if kind == C::CurveCurveFillet {
                fields.extend([
                    ("first_trim_endpoint", IntentLiteral::Enum(key("end"))),
                    ("second_trim_endpoint", IntentLiteral::Enum(key("start"))),
                ]);
            }
            let arc = circular_arc(
                "arc",
                [1.0, 1.0],
                1.0,
                -std::f64::consts::FRAC_PI_2,
                std::f64::consts::PI,
            )
            .with_field(
                IntentFieldKey(key("sweep")),
                IntentLiteral::Enum(key("clockwise")),
            );
            replay(
                kind,
                vec![
                    geometry("line0", segment("line0", [-2.0, 0.0], [2.0, 0.0])),
                    geometry("line1", segment("line1", [0.0, 2.0], [0.0, -2.0])),
                    geometry("arc", arc),
                ],
                [
                    (
                        InputSlot::new(InputRole::Curve, 0),
                        alias("arc", IntentPortRole::Curve),
                    ),
                    (InputSlot::new(InputRole::Span, 0), span(0)),
                    (InputSlot::new(InputRole::Span, 1), span(1)),
                ],
                fields,
            )
        }
    }
}

fn accepted_empty_code_project() -> (
    CodeProject,
    geosolve_sketch_code::ExpandedCodeProject,
    ProjectionalEditorSession,
) {
    let compiled =
        CompiledManagedSource::from_json(EMPTY_MANAGED).expect("clean empty compiler envelope");
    let project = CodeProject::managed(ProjectKey("named-constraint-roundtrip".into()), compiled)
        .expect("empty managed project");
    let desired = required_generated_members(&project).expect("generated member inventory");
    let generated = KeyedReconcileState::empty()
        .plan(desired, &BTreeSet::new())
        .expect("empty reconciliation")
        .into_staged();
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x89_f004_1000),
        DocumentId(PersistentId::from_u128(0x89_f004_1000)),
        1.0,
    )
    .expect("accepted empty code project");
    (project, materialized.expansion, materialized.editor)
}

fn compact_source(value: &ManagedValue) -> String {
    match value {
        ManagedValue::Null => "null".into(),
        ManagedValue::Bool(value) => value.to_string(),
        ManagedValue::Number(value) => {
            assert!(value.is_finite());
            serde_json::to_string(value).expect("finite number")
        }
        ManagedValue::String(value) => serde_json::to_string(value).expect("string literal"),
        ManagedValue::Unit(value) => format!(
            "{}({})",
            value.unit,
            serde_json::to_string(&value.value).expect("finite unit value")
        ),
        ManagedValue::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(compact_source)
                .collect::<Vec<_>>()
                .join(",")
        ),
        ManagedValue::Object(fields) => format!(
            "{{{}}}",
            fields
                .iter()
                .map(|(name, value)| format!(
                    "{}:{}",
                    serde_json::to_string(name).expect("object key"),
                    compact_source(value)
                ))
                .collect::<Vec<_>>()
                .join(",")
        ),
        ManagedValue::Reference { declaration, path } => {
            let mut source = declaration.0.clone();
            for segment in &path.0 {
                match segment {
                    ManagedPathSegment::Field(field) => {
                        source.push('.');
                        source.push_str(field);
                    }
                    ManagedPathSegment::Index(index) => {
                        source.push('[');
                        source.push_str(&index.to_string());
                        source.push(']');
                    }
                    ManagedPathSegment::Member { member } => {
                        source.push('.');
                        source.push_str(member);
                    }
                }
            }
            source
        }
    }
}

fn assert_finite_managed(value: &ManagedValue) {
    match value {
        ManagedValue::Number(value) => assert!(value.is_finite()),
        ManagedValue::Unit(value) => assert!(value.value.is_finite()),
        ManagedValue::Array(values) => values.iter().for_each(assert_finite_managed),
        ManagedValue::Object(fields) => fields.values().for_each(assert_finite_managed),
        ManagedValue::Null
        | ManagedValue::Bool(_)
        | ManagedValue::String(_)
        | ManagedValue::Reference { .. } => {}
    }
}

fn managed_path_value<'a>(
    arguments: &'a BTreeMap<String, ManagedValue>,
    path: &IntentProjectionPath,
) -> Option<&'a ManagedValue> {
    let (first, tail) = path.segments().split_first()?;
    let IntentProjectionPathSegment::Field(first) = first else {
        return None;
    };
    let mut value = arguments.get(first.as_str())?;
    for segment in tail {
        value = match (segment, value) {
            (IntentProjectionPathSegment::Field(field), ManagedValue::Object(fields)) => {
                fields.get(field.as_str())?
            }
            (IntentProjectionPathSegment::Index(index), ManagedValue::Array(values)) => {
                values.get(usize::from(*index))?
            }
            _ => return None,
        };
    }
    Some(value)
}

fn assert_clean_named_value(kind: ConstraintKind, value: &ManagedValue) {
    match value {
        ManagedValue::Array(values) => {
            for value in values {
                assert_clean_named_value(kind, value);
            }
        }
        ManagedValue::Object(fields) => {
            for (name, value) in fields {
                assert!(
                    !matches!(
                        name.as_str(),
                        "inputs"
                            | "fields"
                            | "values"
                            | "results"
                            | "operationOutputs"
                            | "outputs"
                            | "editLens"
                    ),
                    "{kind:?} leaked transport property `{name}`",
                );
                assert_clean_named_value(kind, value);
            }
        }
        ManagedValue::Number(value) => assert!(value.is_finite(), "{kind:?}"),
        ManagedValue::Unit(value) => assert!(value.value.is_finite(), "{kind:?}"),
        ManagedValue::Null
        | ManagedValue::Bool(_)
        | ManagedValue::String(_)
        | ManagedValue::Reference { .. } => {}
    }
}

fn constraint_contact_count(kind: ConstraintKind) -> usize {
    match kind {
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

fn assert_named_constraint_arguments(
    kind: ConstraintKind,
    node: &IntentNode,
    arguments: &BTreeMap<String, ManagedValue>,
) {
    let family = CODE_AUTHORING_FAMILIES
        .iter()
        .find(|family| family.declaration == CodeAuthoringDeclarationKind::Constraint(kind))
        .unwrap_or_else(|| panic!("{kind:?} has no clean authoring family"));
    assert_eq!(family.availability, CodeAuthoringAvailability::Public);
    let descriptor = resolve_code_authoring_declaration(family.namespace, family.method, 0)
        .unwrap_or_else(|error| panic!("{kind:?} clean descriptor failed: {error}"));

    for input in &descriptor.inputs {
        let value = arguments
            .get(&input.name)
            .unwrap_or_else(|| panic!("{kind:?} lost named input `{}`", input.name));
        assert!(
            matches!(value, ManagedValue::Reference { .. }),
            "{kind:?} input `{}` is not one typed lexical reference: {value:?}",
            input.name,
        );
    }

    for field in node.fields.keys() {
        let name = field.0.as_str();
        if name.starts_with("contact_")
            || name.starts_with("first_contact_")
            || name.starts_with("second_contact_")
        {
            continue;
        }
        let projected = descriptor
            .fields
            .iter()
            .find(|descriptor| descriptor.schema.field == *field)
            .unwrap_or_else(|| panic!("{kind:?} field `{name}` has no clean descriptor"));
        let source_value = managed_path_value(arguments, &projected.path)
            .unwrap_or_else(|| panic!("{kind:?} lost named field `{name}`"));
        assert_finite_managed(source_value);
    }

    match constraint_contact_count(kind) {
        0 => {
            assert!(!arguments.contains_key("contact"), "{kind:?}");
            assert!(!arguments.contains_key("contacts"), "{kind:?}");
        }
        1 => assert!(matches!(
            arguments.get("contact"),
            Some(ManagedValue::Object(_))
        )),
        2 => {
            let Some(ManagedValue::Object(contacts)) = arguments.get("contacts") else {
                panic!("{kind:?} lost named first/second contact state")
            };
            assert!(matches!(
                contacts.get("first"),
                Some(ManagedValue::Object(_))
            ));
            assert!(matches!(
                contacts.get("second"),
                Some(ManagedValue::Object(_))
            ));
        }
        _ => unreachable!(),
    }
    assert_clean_named_value(kind, &ManagedValue::Object(arguments.clone()));
}

fn declaration_source(declaration: &geosolve_sketch_code::EditorSourceDeclarationDraft) -> String {
    format!(
        "const {}=$.{}({},{});",
        declaration.variable,
        declaration.builder_path.join("."),
        serde_json::to_string(&declaration.symbol.0).expect("symbol literal"),
        compact_source(&declaration.arguments),
    )
}

fn semantic_path(path: &IntentProjectionPath) -> String {
    path.segments()
        .iter()
        .map(|segment| match segment {
            IntentProjectionPathSegment::Field(field) => field.as_str().to_owned(),
            IntentProjectionPathSegment::Index(index) => index.to_string(),
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn assert_accepted_authority(kind: ConstraintKind, editor: &ProjectionalEditorSession) {
    let accepted = editor
        .coordinator()
        .accepted_materialization()
        .expect("accepted materialization");
    assert!(accepted.validation.hard_residuals_validated, "{kind:?}");
    assert!(accepted.validation.all_active_features_current, "{kind:?}");
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9),
        "{kind:?}",
    );
    let document = accepted.session.design_document();
    assert!(
        document
            .points()
            .iter()
            .flat_map(|point| point.position)
            .all(f64::is_finite),
        "{kind:?}",
    );
    assert!(
        document
            .scalars()
            .iter()
            .all(|scalar| scalar.value.is_finite()),
        "{kind:?}",
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one owner matrix keeps all 35 persistent constraint reverse-source contracts exhaustive"
)]
fn every_standalone_constraint_reverse_projects_and_host_external_kinds_remain_input_gated() {
    let (project, expansion, accepted) = accepted_empty_code_project();
    assert_eq!(ConstraintKind::ALL.len(), 35, "persistent catalog changed");

    let mut covered = BTreeSet::new();
    let mut replayed = BTreeSet::new();
    let mut external_wire_only = BTreeSet::new();
    for kind in ConstraintKind::ALL {
        assert!(covered.insert(kind), "duplicate catalog row {kind:?}");
        match coverage_case(kind) {
            CoverageCase::ExternalWireOnly(draft) => {
                assert!(matches!(
                    kind,
                    ConstraintKind::ExternalPointCoincident | ConstraintKind::ExternalLineCollinear
                ));
                let outputs = draft
                    .output_descriptors()
                    .unwrap_or_else(|error| panic!("{kind:?} wire schema failed: {error}"));
                assert!(!outputs.is_empty(), "{kind:?} lost its result schema");
                assert_eq!(
                    outputs
                        .iter()
                        .map(|output| (semantic_path(&output.path), output.kind))
                        .collect::<Vec<_>>(),
                    vec![
                        ("constraint".into(), IntentPortKind::Constraint),
                        ("source".into(), IntentPortKind::Source),
                    ],
                    "{kind:?}",
                );
                let schema = IntentNodeKind::Constraint { constraint: kind }.schema(0);
                assert_eq!(
                    schema
                        .inputs
                        .iter()
                        .find(|input| input.role == InputRole::External)
                        .map(|input| (input.minimum, input.maximum)),
                    Some((1, 1)),
                    "{kind:?} must retain one typed external operand",
                );
                let family = CODE_AUTHORING_FAMILIES
                    .iter()
                    .find(|family| {
                        family.declaration == CodeAuthoringDeclarationKind::Constraint(kind)
                    })
                    .unwrap_or_else(|| panic!("{kind:?} has no clean authoring catalog row"));
                assert_eq!(
                    family.availability,
                    CodeAuthoringAvailability::RequiresHostSnapshot,
                    "{kind:?} must fail closed without immutable host authority",
                );
                external_wire_only.insert(kind);
            }
            CoverageCase::Replay(fixture) => {
                let mut candidate = accepted
                    .fork_accepted_authority()
                    .expect("candidate editor fork");
                let mut operations = fixture
                    .geometry
                    .iter()
                    .map(|named| IntentPatchOperation::CreateNode {
                        alias: key(named.alias),
                        draft: Box::new(named.draft.clone()),
                        cell: None,
                    })
                    .collect::<Vec<_>>();
                operations.push(IntentPatchOperation::CreateNode {
                    alias: key("relation"),
                    draft: Box::new(fixture.relation),
                    cell: None,
                });
                let outcome = candidate
                    .apply_patch(IntentPatch::new(
                        candidate.coordinator().intent().identity(),
                        IntentPatchPolicy::RequireAccepted,
                        operations,
                    ))
                    .unwrap_or_else(|error| panic!("{kind:?} fixture did not accept: {error}"));
                let mut selected = fixture
                    .geometry
                    .iter()
                    .map(|named| {
                        EditorBootstrapDeclaration::new(
                            outcome.aliases.node(&key(named.alias)).unwrap_or_else(|| {
                                panic!("{kind:?} lost geometry alias {}", named.alias)
                            }),
                            SemanticSymbol(named.alias.into()),
                        )
                    })
                    .collect::<Vec<_>>();
                let relation_node = outcome
                    .aliases
                    .node(&key("relation"))
                    .unwrap_or_else(|| panic!("{kind:?} lost relation alias"));
                selected.push(EditorBootstrapDeclaration::new(
                    relation_node,
                    SemanticSymbol("relation".into()),
                ));
                let insertion = prepare_editor_declaration_insertions(
                    &project, &expansion, &accepted, &candidate, &selected,
                )
                .unwrap_or_else(|error| panic!("{kind:?} did not reverse-project: {error}"));
                let declaration = insertion
                    .declarations
                    .iter()
                    .find(|declaration| declaration.node == relation_node)
                    .unwrap_or_else(|| panic!("{kind:?} lost its source declaration"));
                assert!(!declaration.suppressed, "{kind:?}");
                let family = CODE_AUTHORING_FAMILIES
                    .iter()
                    .find(|family| {
                        family.declaration == CodeAuthoringDeclarationKind::Constraint(kind)
                    })
                    .unwrap_or_else(|| panic!("{kind:?} has no clean authoring family"));
                assert_eq!(family.availability, CodeAuthoringAvailability::Public);
                assert_eq!(
                    declaration.builder_path,
                    [family.namespace, family.method],
                    "{kind:?}",
                );
                let ManagedValue::Object(arguments) = &declaration.arguments else {
                    panic!("{kind:?} named arguments must be an object")
                };
                let node = candidate
                    .coordinator()
                    .intent()
                    .graph()
                    .node(relation_node)
                    .expect("candidate relation node");
                assert_named_constraint_arguments(kind, node, arguments);
                let source_declaration = declaration_source(declaration);
                assert!(
                    !source_declaration.contains("\"domain\""),
                    "{kind:?} leaked intrinsic topology: {source_declaration}",
                );
                if kind == ConstraintKind::PointOnCurve {
                    assert!(
                        source_declaration.contains("\"range\":{\"lower\":0.25,\"upper\":0.75}"),
                        "authored admissible range was not reverse-projected: {source_declaration}",
                    );
                }
                if kind == ConstraintKind::LineCircleTangency {
                    assert!(
                        source_declaration.contains("\"support\":\"supportingLine\""),
                        "explicit supporting-line choice was not reverse-projected: {source_declaration}",
                    );
                    assert!(
                        !source_declaration.contains("period"),
                        "periodic intrinsic topology leaked into source: {source_declaration}",
                    );
                }
                for forbidden in [
                    ".recipe",
                    "geosolve-intent-recipe-v1",
                    "selector",
                    "slot",
                    "editLens",
                    "operationOutputs",
                    "\"inputs\"",
                    "\"fields\"",
                    "\"values\"",
                    "\"results\"",
                    "\"outputs\"",
                ] {
                    assert!(
                        !source_declaration.contains(forbidden),
                        "{kind:?} leaked `{forbidden}`: {source_declaration}",
                    );
                }
                assert!(
                    source_declaration.len() <= 2_048,
                    "{kind:?} named declaration grew to {} bytes\n{source_declaration}",
                    source_declaration.len(),
                );

                assert_accepted_authority(kind, &candidate);
                replayed.insert(kind);
            }
        }
    }

    assert_eq!(covered.len(), ConstraintKind::ALL.len());
    assert_eq!(
        replayed.len(),
        33,
        "every non-external relation must reverse-project"
    );
    assert_eq!(
        external_wire_only,
        BTreeSet::from([
            ConstraintKind::ExternalPointCoincident,
            ConstraintKind::ExternalLineCollinear,
        ]),
        "only relations which require unavailable host-binding source syntax may be wire-only",
    );
}
