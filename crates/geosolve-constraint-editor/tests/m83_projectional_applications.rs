// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    AuthoringApplication, AuthoringOperand, AuthoringOptions, AuthoringTool,
    ColdIntentMaterializer, ConstraintIntent, DimensionKind as AuthoringDimensionKind,
    IntentInspectorEditTarget, IntentInspectorEditValue, IntentInspectorField, IntentNativeBinding,
    IntentNativeWritableLeaf, ProjectionalAuthoringError, ProjectionalCoordinatorError,
    ProjectionalEditorSession, ProjectionalIntentCoordinator, ResolvedConstraintKind,
    SelectionItem, projectional_application_patch,
};
use geosolve_sketch::{
    ContactAdmissibleRange, CurveSpan, DocumentConstraintDefinition, DocumentDimensionDefinition,
    DocumentElementId, DocumentId, OperationControl, PersistentId,
    SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE, ScalarUnit, SketchBoundStatus,
};
use geosolve_sketch_intent::{
    ConstraintKind, DimensionKind as IntentDimensionKind, GeometryRecipeKind, InputRole, InputSlot,
    IntentAliasMap, IntentFieldKey, IntentKey, IntentLiteral, IntentNodeDraft, IntentNodeKind,
    IntentPatch, IntentPatchOperation, IntentPatchPolicy, IntentPlanDisposition, IntentPortRole,
    IntentPortSelector, IntentSession, IntentSessionId, IntentUnit, LeafField, PatchPortRef,
};

fn key(value: &str) -> IntentKey {
    IntentKey::new(value).unwrap()
}

const fn selector(role: IntentPortRole, index: u16) -> IntentPortSelector {
    IntentPortSelector::Node { role, index }
}

fn coordinate(value: f64) -> IntentLiteral {
    IntentLiteral::Quantity {
        value,
        unit: IntentUnit::Length,
    }
}

fn point(name: &str, position: [f64; 2]) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::SketchPoint,
        },
        key(name),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Primary, 0),
        LeafField::X,
        coordinate(position[0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Primary, 0),
        LeafField::Y,
        coordinate(position[1]),
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
        selector(IntentPortRole::Start, 0),
        LeafField::X,
        coordinate(start[0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Start, 0),
        LeafField::Y,
        coordinate(start[1]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::End, 0),
        LeafField::X,
        coordinate(end[0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::End, 0),
        LeafField::Y,
        coordinate(end[1]),
    )
}

fn circle(name: &str, center: [f64; 2], radius: f64) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::CenterRadiusCircle,
        },
        key(name),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Center, 0),
        LeafField::X,
        coordinate(center[0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Center, 0),
        LeafField::Y,
        coordinate(center[1]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Target, 0),
        LeafField::Value,
        coordinate(radius),
    )
}

fn quadratic(name: &str, start: [f64; 2], control: [f64; 2], end: [f64; 2]) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::QuadraticBezier,
        },
        key(name),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Start, 0),
        LeafField::X,
        coordinate(start[0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Start, 0),
        LeafField::Y,
        coordinate(start[1]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Control, 0),
        LeafField::X,
        coordinate(control[0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Control, 0),
        LeafField::Y,
        coordinate(control[1]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::End, 0),
        LeafField::X,
        coordinate(end[0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::End, 0),
        LeafField::Y,
        coordinate(end[1]),
    )
}

fn coordinator(raw: u128) -> ProjectionalIntentCoordinator {
    ProjectionalIntentCoordinator::empty(
        IntentSessionId::from_raw(raw),
        ColdIntentMaterializer::with_default_policy(
            DocumentId(PersistentId::from_u128(raw << 32)),
            1.0,
        )
        .unwrap(),
    )
    .unwrap()
}

fn create(
    coordinator: &mut ProjectionalIntentCoordinator,
    declarations: impl IntoIterator<Item = (&'static str, IntentNodeDraft)>,
) -> IntentAliasMap {
    let operations = declarations
        .into_iter()
        .map(|(alias, draft)| IntentPatchOperation::CreateNode {
            alias: key(alias),
            draft: Box::new(draft),
            cell: None,
        })
        .collect();
    let outcome = coordinator
        .apply_patch(IntentPatch::new(
            coordinator.intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            operations,
        ))
        .unwrap();
    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
    outcome.aliases
}

fn span(coordinator: &ProjectionalIntentCoordinator, index: usize) -> CurveSpan {
    let document = coordinator
        .accepted_materialization()
        .unwrap()
        .session
        .design_document();
    let curve = document.curves()[index].id;
    document.curve_spans(curve).unwrap()[0]
}

fn alias_binding(
    coordinator: &ProjectionalIntentCoordinator,
    aliases: &IntentAliasMap,
    alias: &str,
    role: IntentPortRole,
) -> IntentNativeBinding {
    let port = aliases.port(&key(alias), selector(role, 0)).unwrap();
    coordinator
        .accepted_materialization()
        .unwrap()
        .ownership
        .port(port)
        .unwrap()
}

fn translate(
    coordinator: &ProjectionalIntentCoordinator,
    application: &AuthoringApplication,
) -> Result<geosolve_constraint_editor::ProjectionalApplicationPatch, ProjectionalAuthoringError> {
    let accepted = coordinator.accepted_materialization().unwrap();
    projectional_application_patch(
        coordinator.intent().identity(),
        coordinator.intent(),
        &accepted.ownership,
        accepted.session.design_document(),
        accepted
            .session
            .accepted_state_for_current_input()
            .unwrap()
            .document(),
        application,
    )
}

fn relation(
    tool: ConstraintIntent,
    operands: Vec<AuthoringOperand>,
    resolved: ResolvedConstraintKind,
) -> AuthoringApplication {
    AuthoringApplication {
        tool: AuthoringTool::Constraint(tool),
        operands,
        options: AuthoringOptions::default(),
        resolved_constraint: Some(resolved),
    }
}

fn translated_kind(
    coordinator: &ProjectionalIntentCoordinator,
    application: &AuthoringApplication,
) -> IntentNodeKind {
    let translated = translate(coordinator, application)
        .unwrap_or_else(|error| panic!("{application:?}: {error:?}"));
    translated
        .patch
        .operations()
        .iter()
        .find_map(|operation| match operation {
            IntentPatchOperation::CreateNode { draft, .. } => Some(draft.kind.clone()),
            _ => None,
        })
        .unwrap()
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one closed inventory proves every contextual relation and dimension bridge"
)]
fn complete_contextual_authoring_inventory_translates_to_closed_intent_kinds() {
    use geosolve_sketch::SketchDatum;

    let mut coordinator = coordinator(0x8300_7200);
    let aliases = create(
        &mut coordinator,
        [
            ("p0", point("p0", [-3.0, 2.0])),
            ("p1", point("p1", [3.0, 2.0])),
            ("line0", segment("line0", [0.0, 0.0], [4.0, 0.0])),
            ("line1", segment("line1", [0.0, 0.0], [0.0, 3.0])),
            ("circle0", circle("circle0", [-4.0, -3.0], 1.0)),
            ("circle1", circle("circle1", [4.0, -3.0], 2.0)),
            (
                "curve0",
                quadratic("curve0", [-4.0, 5.0], [-2.0, 7.0], [0.0, 5.0]),
            ),
            (
                "curve1",
                quadratic("curve1", [0.0, 5.0], [2.0, 3.0], [4.0, 5.0]),
            ),
        ],
    );
    let IntentNativeBinding::Point(p0) =
        alias_binding(&coordinator, &aliases, "p0", IntentPortRole::Primary)
    else {
        panic!("p0 must own a point");
    };
    let IntentNativeBinding::Point(p1) =
        alias_binding(&coordinator, &aliases, "p1", IntentPortRole::Primary)
    else {
        panic!("p1 must own a point");
    };
    let bound_span = |alias| {
        let IntentNativeBinding::CurveSpan(span) =
            alias_binding(&coordinator, &aliases, alias, IntentPortRole::Span)
        else {
            panic!("{alias} must own a span");
        };
        span
    };
    let line0 = bound_span("line0");
    let line1 = bound_span("line1");
    let circle0 = bound_span("circle0");
    let circle1 = bound_span("circle1");
    let curve0 = bound_span("curve0");
    let curve1 = bound_span("curve1");
    let picked = |item| AuthoringOperand::picked(SelectionItem::Curve(item), Some(0.5));
    let selected = |item| AuthoringOperand::selected(item);
    let cases = vec![
        (
            relation(
                ConstraintIntent::Lock,
                vec![selected(SelectionItem::Point(p0))],
                ResolvedConstraintKind::FixedPoint,
            ),
            ConstraintKind::FixedPoint,
        ),
        (
            relation(
                ConstraintIntent::Coincident,
                vec![
                    selected(SelectionItem::Point(p0)),
                    selected(SelectionItem::Datum(SketchDatum::Origin)),
                ],
                ResolvedConstraintKind::CoincidentWithOrigin,
            ),
            ConstraintKind::CoincidentWithOrigin,
        ),
        (
            relation(
                ConstraintIntent::Coincident,
                vec![
                    selected(SelectionItem::Point(p0)),
                    selected(SelectionItem::Datum(SketchDatum::XAxis)),
                ],
                ResolvedConstraintKind::PointOnDatumAxis,
            ),
            ConstraintKind::PointOnDatumAxis,
        ),
        (
            relation(
                ConstraintIntent::Coincident,
                vec![
                    selected(SelectionItem::Point(p0)),
                    selected(SelectionItem::Point(p1)),
                ],
                ResolvedConstraintKind::CoincidentPoints,
            ),
            ConstraintKind::Coincident,
        ),
        (
            relation(
                ConstraintIntent::Coincident,
                vec![selected(SelectionItem::Point(p0)), picked(line0)],
                ResolvedConstraintKind::PointOnCurve,
            ),
            ConstraintKind::PointOnCurve,
        ),
        (
            relation(
                ConstraintIntent::Coincident,
                vec![picked(curve0), picked(curve1)],
                ResolvedConstraintKind::CurveContact,
            ),
            ConstraintKind::CurveCurveContact,
        ),
        (
            relation(
                ConstraintIntent::Horizontal,
                vec![selected(SelectionItem::Curve(line0))],
                ResolvedConstraintKind::HorizontalLine,
            ),
            ConstraintKind::Horizontal,
        ),
        (
            relation(
                ConstraintIntent::Vertical,
                vec![selected(SelectionItem::Curve(line1))],
                ResolvedConstraintKind::VerticalLine,
            ),
            ConstraintKind::Vertical,
        ),
        (
            relation(
                ConstraintIntent::Horizontal,
                vec![
                    selected(SelectionItem::Point(p0)),
                    selected(SelectionItem::Point(p1)),
                ],
                ResolvedConstraintKind::HorizontalPoints,
            ),
            ConstraintKind::HorizontalPoints,
        ),
        (
            relation(
                ConstraintIntent::Vertical,
                vec![
                    selected(SelectionItem::Point(p0)),
                    selected(SelectionItem::Point(p1)),
                ],
                ResolvedConstraintKind::VerticalPoints,
            ),
            ConstraintKind::VerticalPoints,
        ),
        (
            relation(
                ConstraintIntent::Concentric,
                vec![
                    selected(SelectionItem::Curve(circle0)),
                    selected(SelectionItem::Curve(circle1)),
                ],
                ResolvedConstraintKind::ConcentricCurves,
            ),
            ConstraintKind::Concentric,
        ),
        (
            relation(
                ConstraintIntent::Collinear,
                vec![
                    selected(SelectionItem::Curve(line0)),
                    selected(SelectionItem::Curve(line1)),
                ],
                ResolvedConstraintKind::CollinearSupports,
            ),
            ConstraintKind::Collinear,
        ),
        (
            relation(
                ConstraintIntent::Collinear,
                vec![
                    selected(SelectionItem::Curve(line0)),
                    selected(SelectionItem::Datum(SketchDatum::XAxis)),
                ],
                ResolvedConstraintKind::CollinearWithDatumAxis,
            ),
            ConstraintKind::CollinearWithDatumAxis,
        ),
        (
            relation(
                ConstraintIntent::Parallel,
                vec![
                    selected(SelectionItem::Curve(line0)),
                    selected(SelectionItem::Curve(line1)),
                ],
                ResolvedConstraintKind::ParallelLines,
            ),
            ConstraintKind::Parallel,
        ),
        (
            relation(
                ConstraintIntent::Perpendicular,
                vec![
                    selected(SelectionItem::Curve(line0)),
                    selected(SelectionItem::Curve(line1)),
                ],
                ResolvedConstraintKind::PerpendicularLines,
            ),
            ConstraintKind::Perpendicular,
        ),
        (
            relation(
                ConstraintIntent::Perpendicular,
                vec![
                    selected(SelectionItem::Curve(line0)),
                    selected(SelectionItem::Curve(circle0)),
                ],
                ResolvedConstraintKind::RadialLine,
            ),
            ConstraintKind::PointOnCurve,
        ),
        (
            relation(
                ConstraintIntent::Equal,
                vec![
                    selected(SelectionItem::Curve(line0)),
                    selected(SelectionItem::Curve(line1)),
                ],
                ResolvedConstraintKind::EqualLength,
            ),
            ConstraintKind::EqualLength,
        ),
        (
            relation(
                ConstraintIntent::Equal,
                vec![
                    selected(SelectionItem::Curve(circle0)),
                    selected(SelectionItem::Curve(circle1)),
                ],
                ResolvedConstraintKind::EqualRadius,
            ),
            ConstraintKind::EqualRadius,
        ),
        (
            relation(
                ConstraintIntent::Equal,
                vec![picked(curve0), picked(curve1)],
                ResolvedConstraintKind::EqualCurvature,
            ),
            ConstraintKind::EqualCurvature,
        ),
        (
            relation(
                ConstraintIntent::Midpoint,
                vec![
                    selected(SelectionItem::Point(p0)),
                    selected(SelectionItem::Curve(line0)),
                ],
                ResolvedConstraintKind::Midpoint,
            ),
            ConstraintKind::Midpoint,
        ),
        (
            relation(
                ConstraintIntent::Symmetric,
                vec![
                    selected(SelectionItem::Point(p0)),
                    selected(SelectionItem::Point(p1)),
                    selected(SelectionItem::Curve(line0)),
                ],
                ResolvedConstraintKind::SymmetricAboutLine,
            ),
            ConstraintKind::SymmetricAboutLine,
        ),
        (
            relation(
                ConstraintIntent::Symmetric,
                vec![
                    selected(SelectionItem::Point(p0)),
                    selected(SelectionItem::Point(p1)),
                    selected(SelectionItem::Datum(SketchDatum::YAxis)),
                ],
                ResolvedConstraintKind::SymmetricAboutDatumAxis,
            ),
            ConstraintKind::SymmetricAboutDatumAxis,
        ),
        (
            relation(
                ConstraintIntent::Tangent,
                vec![picked(curve0), picked(curve1)],
                ResolvedConstraintKind::CurveTangency,
            ),
            ConstraintKind::CurveCurveTangency,
        ),
        (
            relation(
                ConstraintIntent::Continuity,
                vec![
                    AuthoringOperand::picked(SelectionItem::Curve(curve0), Some(1.0)),
                    AuthoringOperand::picked(SelectionItem::Curve(curve1), Some(0.0)),
                ],
                ResolvedConstraintKind::EndpointContinuity,
            ),
            ConstraintKind::EndpointContinuity,
        ),
    ];
    assert_eq!(cases.len(), 24);
    for (application, expected) in cases {
        assert_eq!(
            translated_kind(&coordinator, &application),
            IntentNodeKind::Constraint {
                constraint: expected
            },
            "{expected:?}"
        );
    }

    let dimension_cases = [
        (
            AuthoringDimensionKind::PointDistance,
            vec![
                selected(SelectionItem::Point(p0)),
                selected(SelectionItem::Point(p1)),
            ],
            IntentDimensionKind::PointDistance,
        ),
        (
            AuthoringDimensionKind::SegmentLength,
            vec![selected(SelectionItem::Curve(line0))],
            IntentDimensionKind::CurveLength,
        ),
        (
            AuthoringDimensionKind::Radius,
            vec![selected(SelectionItem::Curve(circle0))],
            IntentDimensionKind::Radius,
        ),
        (
            AuthoringDimensionKind::Diameter,
            vec![selected(SelectionItem::Curve(circle0))],
            IntentDimensionKind::Diameter,
        ),
        (
            AuthoringDimensionKind::OrientedAngle,
            vec![
                selected(SelectionItem::Curve(line0)),
                selected(SelectionItem::Curve(line1)),
            ],
            IntentDimensionKind::OrientedAngle,
        ),
    ];
    for (kind, operands, expected) in dimension_cases {
        let application = AuthoringApplication {
            tool: AuthoringTool::Dimension(kind),
            operands,
            options: AuthoringOptions::default(),
            resolved_constraint: None,
        };
        assert_eq!(
            translated_kind(&coordinator, &application),
            IntentNodeKind::Dimension {
                dimension: expected
            },
            "{expected:?}"
        );
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one end-to-end transaction test keeps relation, dimension, history and native ownership together"
)]
fn relation_and_dimension_authoring_share_one_typed_intent_history() {
    let mut coordinator = coordinator(0x8300_7201);
    create(
        &mut coordinator,
        [("edge", segment("edge", [0.0, 0.0], [4.0, 0.0]))],
    );
    let edge = span(&coordinator, 0);
    let initial_history = coordinator.intent().history_projection().applied.len();

    let horizontal = relation(
        ConstraintIntent::Horizontal,
        vec![AuthoringOperand::selected(SelectionItem::Curve(edge))],
        ResolvedConstraintKind::HorizontalLine,
    );
    let translated = translate(&coordinator, &horizontal).unwrap();
    assert_eq!(
        translated.patch.policy,
        IntentPatchPolicy::RetainFailedIntent
    );
    let relation_alias = translated.declaration_alias.clone();
    let outcome = coordinator.apply_patch(translated.patch).unwrap();
    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
    let relation_port = outcome
        .aliases
        .port(&relation_alias, selector(IntentPortRole::Constraint, 0))
        .unwrap();
    assert!(matches!(
        coordinator
            .accepted_materialization()
            .unwrap()
            .ownership
            .port(relation_port),
        Some(IntentNativeBinding::Constraint(_))
    ));
    assert!(
        coordinator
            .accepted_materialization()
            .unwrap()
            .session
            .design_document()
            .constraints()
            .iter()
            .any(|constraint| matches!(
                constraint.definition,
                DocumentConstraintDefinition::Horizontal { line } if line == edge
            ))
    );

    let dimension = AuthoringApplication {
        tool: AuthoringTool::Dimension(AuthoringDimensionKind::SegmentLength),
        operands: vec![AuthoringOperand::selected(SelectionItem::Curve(edge))],
        options: AuthoringOptions::default(),
        resolved_constraint: None,
    };
    let translated = translate(&coordinator, &dimension).unwrap();
    let dimension_alias = translated.declaration_alias.clone();
    let outcome = coordinator.apply_patch(translated.patch).unwrap();
    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
    let dimension_port = outcome
        .aliases
        .port(&dimension_alias, selector(IntentPortRole::Dimension, 0))
        .unwrap();
    assert!(matches!(
        coordinator
            .accepted_materialization()
            .unwrap()
            .ownership
            .port(dimension_port),
        Some(IntentNativeBinding::Dimension(_))
    ));
    let document = coordinator
        .accepted_materialization()
        .unwrap()
        .session
        .design_document();
    let DocumentDimensionDefinition::CurveLength { curve, target } =
        document.dimensions()[0].definition
    else {
        panic!("segment authoring must create one native curve-length dimension");
    };
    assert_eq!(curve, edge);
    assert_eq!(
        document.scalar(target).unwrap().value.to_bits(),
        4.0_f64.to_bits()
    );
    assert_eq!(
        coordinator.intent().history_projection().applied.len(),
        initial_history + 2
    );

    coordinator.undo().unwrap().unwrap();
    assert!(
        coordinator
            .accepted_materialization()
            .unwrap()
            .session
            .design_document()
            .dimensions()
            .is_empty()
    );
    coordinator.redo().unwrap().unwrap();
    assert_eq!(
        coordinator
            .accepted_materialization()
            .unwrap()
            .session
            .design_document()
            .dimensions()
            .len(),
        1
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one direct-manipulation matrix keeps constrained motion, driver and branch invariants contiguous"
)]
fn constrained_endpoint_drag_changes_only_its_reverse_leaves_and_never_rewrites_drivers() {
    let mut coordinator = coordinator(0x8300_7208);
    let aliases = create(
        &mut coordinator,
        [(
            "edge",
            segment("edge", [0.0, 0.0], [4.0, 0.0]).with_field(
                IntentFieldKey(key("branch_direction")),
                IntentLiteral::Point([1.0, 0.0]),
            ),
        )],
    );
    let edge_node = aliases.node(&key("edge")).unwrap();
    let IntentNativeBinding::Point(start) =
        alias_binding(&coordinator, &aliases, "edge", IntentPortRole::Start)
    else {
        panic!("segment start must own one native point")
    };
    let IntentNativeBinding::Point(end) =
        alias_binding(&coordinator, &aliases, "edge", IntentPortRole::End)
    else {
        panic!("segment end must own one native point")
    };
    let edge = span(&coordinator, 0);

    let horizontal = translate(
        &coordinator,
        &relation(
            ConstraintIntent::Horizontal,
            vec![AuthoringOperand::selected(SelectionItem::Curve(edge))],
            ResolvedConstraintKind::HorizontalLine,
        ),
    )
    .unwrap();
    assert_eq!(
        coordinator
            .apply_patch(horizontal.patch)
            .unwrap()
            .disposition,
        IntentPlanDisposition::Accepted,
    );
    let dimension = translate(
        &coordinator,
        &AuthoringApplication {
            tool: AuthoringTool::Dimension(AuthoringDimensionKind::SegmentLength),
            operands: vec![AuthoringOperand::selected(SelectionItem::Curve(edge))],
            options: AuthoringOptions::default(),
            resolved_constraint: None,
        },
    )
    .unwrap();
    assert_eq!(
        coordinator
            .apply_patch(dimension.patch)
            .unwrap()
            .disposition,
        IntentPlanDisposition::Accepted,
    );

    let ownership = &coordinator.accepted_materialization().unwrap().ownership;
    let start_x = ownership
        .writable_leaf(IntentNativeWritableLeaf::PointX { point: start })
        .unwrap();
    let start_y = ownership
        .writable_leaf(IntentNativeWritableLeaf::PointY { point: start })
        .unwrap();
    let end_x = ownership
        .writable_leaf(IntentNativeWritableLeaf::PointX { point: end })
        .unwrap();
    let end_y = ownership
        .writable_leaf(IntentNativeWritableLeaf::PointY { point: end })
        .unwrap();
    let branch_before = coordinator.intent().graph().node(edge_node).unwrap().fields
        [&IntentFieldKey(key("branch_direction"))]
        .clone();
    let instance_before = coordinator.intent().instance().values().clone();
    let document = coordinator
        .accepted_materialization()
        .unwrap()
        .session
        .design_document();
    let DocumentDimensionDefinition::CurveLength { target, .. } =
        document.dimensions()[0].definition
    else {
        panic!("fixture must own one driving curve-length dimension")
    };
    let driver_before = document.scalar(target).unwrap().value;

    coordinator.begin_point_drag(83, end).unwrap();
    let preview = coordinator
        .preview_point_drag(83, 1, [6.0, 2.0], OperationControl::unlimited())
        .unwrap()
        .expect("the constrained endpoint still has rigid translation freedom");
    assert_eq!(
        preview.accepted_position.map(f64::to_bits),
        [6.0, 2.0].map(f64::to_bits)
    );
    coordinator.finish_point_drag(83, 1).unwrap();

    let instance_after = coordinator.intent().instance().values();
    let changed = instance_before
        .iter()
        .filter_map(|(leaf, value)| (instance_after.get(leaf) != Some(value)).then_some(*leaf))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        changed,
        [start_x, start_y, end_x, end_y].into_iter().collect(),
        "the translated rigid segment may rewrite only its four free placement leaves",
    );
    assert_eq!(
        coordinator.intent().graph().node(edge_node).unwrap().fields
            [&IntentFieldKey(key("branch_direction"))],
        branch_before,
    );
    let accepted = coordinator.accepted_materialization().unwrap();
    let accepted_document = accepted
        .session
        .accepted_state_for_current_input()
        .unwrap()
        .document();
    assert_eq!(
        accepted_document.scalar(target).unwrap().value.to_bits(),
        driver_before.to_bits(),
    );
    assert_eq!(
        accepted_document
            .point(end)
            .unwrap()
            .position
            .map(f64::to_bits),
        [6.0, 2.0].map(f64::to_bits),
    );
    let fixed_start = accepted_document.point(start).unwrap().position;
    assert!((fixed_start[0] - 2.0).abs() <= 1.0e-12);
    assert!((fixed_start[1] - 2.0).abs() <= 1.0e-12);
    assert!(accepted.validation.hard_residuals_validated);

    let lock = translate(
        &coordinator,
        &relation(
            ConstraintIntent::Lock,
            vec![AuthoringOperand::selected(SelectionItem::Point(start))],
            ResolvedConstraintKind::FixedPoint,
        ),
    )
    .unwrap();
    assert_eq!(
        coordinator.apply_patch(lock.patch).unwrap().disposition,
        IntentPlanDisposition::Accepted,
    );
    let fixed_identity = coordinator.intent().identity();
    let fixed_history = coordinator.intent().history_projection();
    let fixed_document = coordinator
        .accepted_materialization()
        .unwrap()
        .session
        .design_document()
        .clone();
    coordinator.begin_point_drag(84, start).unwrap();
    let fixed_preview = coordinator
        .preview_point_drag(84, 2, [10.0, 10.0], OperationControl::unlimited())
        .unwrap();
    if let Some(preview) = fixed_preview {
        assert_eq!(
            preview.accepted_position.map(f64::to_bits),
            fixed_start.map(f64::to_bits),
        );
        assert!(matches!(
            coordinator.finish_point_drag(84, 2),
            Err(ProjectionalCoordinatorError::DragDidNotMove)
        ));
    } else {
        coordinator.cancel_point_drag();
    }
    assert_eq!(coordinator.intent().identity(), fixed_identity);
    assert_eq!(coordinator.intent().history_projection(), fixed_history);
    assert_eq!(
        coordinator
            .accepted_materialization()
            .unwrap()
            .session
            .design_document(),
        &fixed_document,
    );
}

#[test]
fn point_curve_application_preserves_picked_contact_metadata() {
    let mut coordinator = coordinator(0x8300_7202);
    create(
        &mut coordinator,
        [
            ("point", point("point", [1.0, 2.0])),
            ("edge", segment("edge", [0.0, 0.0], [4.0, 0.0])),
        ],
    );
    let document = coordinator
        .accepted_materialization()
        .unwrap()
        .session
        .design_document();
    let point = document.points()[0].id;
    let edge = span(&coordinator, 0);
    let application = relation(
        ConstraintIntent::Coincident,
        vec![
            AuthoringOperand::selected(SelectionItem::Point(point)),
            AuthoringOperand::picked(SelectionItem::Curve(edge), Some(0.25)),
        ],
        ResolvedConstraintKind::PointOnCurve,
    );
    let translated = translate(&coordinator, &application).unwrap();
    let draft = translated
        .patch
        .operations()
        .iter()
        .find_map(|operation| match operation {
            IntentPatchOperation::CreateNode { draft, .. } => Some(draft.as_ref()),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        draft.kind,
        IntentNodeKind::Constraint {
            constraint: ConstraintKind::PointOnCurve
        }
    );
    assert_eq!(
        draft
            .fields
            .get(&geosolve_sketch_intent::IntentFieldKey(key(
                "contact_parameter"
            ))),
        Some(&IntentLiteral::Quantity {
            value: 0.25,
            unit: IntentUnit::Dimensionless,
        })
    );
    assert!(
        !draft
            .fields
            .keys()
            .any(|field| field.0.as_str().contains("domain"))
    );
    assert!(
        draft
            .fields
            .contains_key(&geosolve_sketch_intent::IntentFieldKey(key(
                "contact_neighborhood"
            )))
    );

    let outcome = coordinator.apply_patch(translated.patch).unwrap();
    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
    let document = coordinator
        .accepted_materialization()
        .unwrap()
        .session
        .design_document();
    let DocumentConstraintDefinition::PointOnCurve {
        point: actual,
        contact,
    } = document.constraints()[0].definition
    else {
        panic!("contextual Coincident must lower to point-on-curve");
    };
    assert_eq!(actual, point);
    assert_eq!(document.contact(contact).unwrap().curve, edge);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one retained-coordinator regression keeps the structural range edit, accepted-scene continuation, locality, and bound evidence together"
)]
fn structural_contact_range_edit_continues_the_accepted_scene_to_the_new_bound() {
    let mut coordinator = coordinator(0x8300_7203);
    let contact = IntentNodeDraft::new(
        IntentNodeKind::Constraint {
            constraint: ConstraintKind::PointOnCurve,
        },
        key("contact"),
    )
    .with_input(
        InputSlot::new(InputRole::Point, 0),
        PatchPortRef::Alias {
            node: key("contact_point"),
            selector: selector(IntentPortRole::Primary, 0),
        },
    )
    .with_input(
        InputSlot::new(InputRole::Span, 0),
        PatchPortRef::Alias {
            node: key("line"),
            selector: selector(IntentPortRole::Span, 0),
        },
    )
    .with_field(
        IntentFieldKey(key("contact_parameter")),
        IntentLiteral::Quantity {
            value: 0.8,
            unit: IntentUnit::Dimensionless,
        },
    );
    let aliases = create(
        &mut coordinator,
        [
            ("line", segment("line", [0.0, 0.0], [10.0, 0.0])),
            ("contact_point", point("contact_point", [8.0, 0.0])),
            ("unrelated", point("unrelated", [17.0, -9.0])),
            ("contact", contact),
        ],
    );
    let relation_node = aliases.node(&key("contact")).unwrap();
    let IntentNativeBinding::Contact(contact_id) =
        alias_binding(&coordinator, &aliases, "contact", IntentPortRole::Contact)
    else {
        panic!("point-on-curve relation must own its contact");
    };
    let IntentNativeBinding::Point(unrelated_id) =
        alias_binding(&coordinator, &aliases, "unrelated", IntentPortRole::Primary)
    else {
        panic!("unrelated declaration must own its point");
    };
    let before = coordinator
        .accepted_materialization()
        .unwrap()
        .session
        .accepted_state_for_current_input()
        .unwrap()
        .document()
        .clone();
    let before_contact = before.contact(contact_id).unwrap();
    assert!(before.scalar(before_contact.parameter).unwrap().value > 0.5);
    let unrelated_before = before.point(unrelated_id).unwrap().position;

    let outcome = coordinator
        .apply_patch(IntentPatch::new(
            coordinator.intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![
                IntentPatchOperation::SetDefinitionField {
                    node: relation_node,
                    field: IntentFieldKey(key("contact_range_lower")),
                    value: IntentLiteral::Quantity {
                        value: 0.0,
                        unit: IntentUnit::Dimensionless,
                    },
                },
                IntentPatchOperation::SetDefinitionField {
                    node: relation_node,
                    field: IntentFieldKey(key("contact_range_upper")),
                    value: IntentLiteral::Quantity {
                        value: 0.5,
                        unit: IntentUnit::Dimensionless,
                    },
                },
            ],
        ))
        .expect("range-only source edit must publish");
    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);

    let accepted = coordinator
        .accepted_materialization()
        .unwrap()
        .session
        .accepted_state_for_current_input()
        .unwrap();
    let contact = accepted.document().contact(contact_id).unwrap();
    assert_eq!(
        contact.admissible_range,
        Some(ContactAdmissibleRange {
            lower: 0.0,
            upper: 0.5,
        })
    );
    assert_eq!(
        accepted
            .document()
            .scalar(contact.parameter)
            .unwrap()
            .value
            .to_bits(),
        0.5_f64.to_bits()
    );
    assert_eq!(
        accepted
            .document()
            .point(unrelated_id)
            .unwrap()
            .position
            .map(f64::to_bits),
        unrelated_before.map(f64::to_bits)
    );
    let diagnostics = accepted.diagnostics();
    let solve = diagnostics.solve.as_ref().unwrap();
    assert!(solve.accepted && solve.hard_residuals_validated);
    assert!(
        solve
            .maximum_normalized_hard_residual
            .is_none_or(|value| value <= SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE)
    );
    let bound = diagnostics
        .bounds
        .iter()
        .find(|bound| bound.target == DocumentElementId::Contact(contact_id))
        .unwrap();
    assert_eq!(bound.status, SketchBoundStatus::ActiveUpper);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one periodic-contact drag keeps unit, validation, metadata, history, and Undo invariants together"
)]
fn circle_point_on_curve_drag_round_trips_native_angle_as_dimensionless_parameter() {
    let mut coordinator = coordinator(0x8300_9005);
    let aliases = create(
        &mut coordinator,
        [
            ("point", point("point", [1.0, 0.0])),
            ("circle", circle("circle", [0.0, 0.0], 1.0)),
        ],
    );
    let IntentNativeBinding::Point(point) =
        alias_binding(&coordinator, &aliases, "point", IntentPortRole::Primary)
    else {
        panic!("point declaration must own one native point");
    };
    let IntentNativeBinding::CurveSpan(circle) =
        alias_binding(&coordinator, &aliases, "circle", IntentPortRole::Span)
    else {
        panic!("circle declaration must own one native span");
    };
    let translated = translate(
        &coordinator,
        &relation(
            ConstraintIntent::Coincident,
            vec![
                AuthoringOperand::selected(SelectionItem::Point(point)),
                AuthoringOperand::picked(SelectionItem::Curve(circle), Some(0.0)),
            ],
            ResolvedConstraintKind::PointOnCurve,
        ),
    )
    .unwrap();
    coordinator.apply_patch(translated.patch).unwrap();

    let accepted_before = coordinator
        .accepted_materialization()
        .unwrap()
        .session
        .accepted_state_for_current_input()
        .unwrap()
        .document()
        .clone();
    let DocumentConstraintDefinition::PointOnCurve { contact, .. } =
        accepted_before.constraints()[0].definition
    else {
        panic!("fixture must lower one Point-on-Curve relation");
    };
    let contact_before = accepted_before.contact(contact).unwrap().clone();
    let parameter = contact_before.parameter;
    let parameter_leaf = coordinator
        .accepted_materialization()
        .unwrap()
        .ownership
        .writable_leaf(IntentNativeWritableLeaf::ScalarValue { scalar: parameter })
        .expect("contact parameter must retain a writable reverse owner");
    let history_before = coordinator.intent().undo_len();

    coordinator.begin_point_drag(90_005, point).unwrap();
    let preview = coordinator
        .preview_point_drag(90_005, 1, [0.0, 1.0], OperationControl::unlimited())
        .unwrap()
        .expect("circle-constrained point must have one accepted preview");
    assert!(preview.accepted_position.into_iter().all(f64::is_finite));
    let outcome = coordinator.finish_point_drag(90_005, 1).unwrap();

    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
    assert_eq!(coordinator.intent().undo_len(), history_before + 1);
    assert!(matches!(
        coordinator.intent().instance().values().get(&parameter_leaf),
        Some(IntentLiteral::Quantity {
            value,
            unit: IntentUnit::Dimensionless,
        }) if value.is_finite()
    ));
    let accepted = coordinator.accepted_materialization().unwrap();
    assert!(accepted.validation.hard_residuals_validated);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
    );
    let document = accepted
        .session
        .accepted_state_for_current_input()
        .unwrap()
        .document();
    assert!(
        document
            .points()
            .iter()
            .flat_map(|point| point.position)
            .chain(document.scalars().iter().map(|scalar| scalar.value))
            .all(f64::is_finite)
    );
    let contact_after = document.contact(contact).unwrap();
    assert_eq!(contact_after.domain, contact_before.domain);
    assert_eq!(contact_after.winding, contact_before.winding);
    assert_eq!(contact_after.neighborhood, contact_before.neighborhood);
    assert_eq!(
        contact_after.tangent_orientation,
        contact_before.tangent_orientation
    );

    coordinator.undo().unwrap().expect("drag must be undoable");
    assert_eq!(
        coordinator
            .accepted_materialization()
            .unwrap()
            .session
            .accepted_state_for_current_input()
            .unwrap()
            .document(),
        &accepted_before
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one mixed-contact drag keeps both authored/native unit boundaries and transaction invariants together"
)]
fn line_circle_tangency_drag_round_trips_mixed_contact_units_and_radius() {
    let mut coordinator = coordinator(0x8300_9006);
    let aliases = create(
        &mut coordinator,
        [
            (
                "line",
                segment("line", [2.0, 1.0], [-2.0, 1.0]).with_field(
                    IntentFieldKey(key("branch_direction")),
                    IntentLiteral::Point([-1.0, 0.0]),
                ),
            ),
            ("circle", circle("circle", [0.0, 0.0], 1.0)),
        ],
    );
    let IntentNativeBinding::Point(line_start) =
        alias_binding(&coordinator, &aliases, "line", IntentPortRole::Start)
    else {
        panic!("line declaration must own one native start point");
    };
    let IntentNativeBinding::Point(line_end) =
        alias_binding(&coordinator, &aliases, "line", IntentPortRole::End)
    else {
        panic!("line declaration must own one native end point");
    };
    let IntentNativeBinding::CurveSpan(line) =
        alias_binding(&coordinator, &aliases, "line", IntentPortRole::Span)
    else {
        panic!("line declaration must own one native span");
    };
    let IntentNativeBinding::Point(circle_center) =
        alias_binding(&coordinator, &aliases, "circle", IntentPortRole::Center)
    else {
        panic!("circle declaration must own one native center point");
    };
    let IntentNativeBinding::Scalar(radius) =
        alias_binding(&coordinator, &aliases, "circle", IntentPortRole::Target)
    else {
        panic!("circle declaration must own one native radius scalar");
    };
    let IntentNativeBinding::CurveSpan(circle) =
        alias_binding(&coordinator, &aliases, "circle", IntentPortRole::Span)
    else {
        panic!("circle declaration must own one native span");
    };

    for point in [line_start, circle_center] {
        let lock = translate(
            &coordinator,
            &relation(
                ConstraintIntent::Lock,
                vec![AuthoringOperand::selected(SelectionItem::Point(point))],
                ResolvedConstraintKind::FixedPoint,
            ),
        )
        .unwrap();
        assert_eq!(
            coordinator.apply_patch(lock.patch).unwrap().disposition,
            IntentPlanDisposition::Accepted,
        );
    }
    let tangency = translate(
        &coordinator,
        &relation(
            ConstraintIntent::Tangent,
            vec![
                AuthoringOperand::picked(SelectionItem::Curve(line), Some(0.5)),
                AuthoringOperand::picked(
                    SelectionItem::Curve(circle),
                    Some(std::f64::consts::FRAC_PI_2),
                ),
            ],
            ResolvedConstraintKind::CurveTangency,
        ),
    )
    .unwrap();
    assert_eq!(
        coordinator.apply_patch(tangency.patch).unwrap().disposition,
        IntentPlanDisposition::Accepted,
    );

    let accepted_before = coordinator
        .accepted_materialization()
        .unwrap()
        .session
        .accepted_state_for_current_input()
        .unwrap()
        .document()
        .clone();
    let (first_contact, second_contact) = accepted_before
        .constraints()
        .iter()
        .find_map(|constraint| match &constraint.definition {
            DocumentConstraintDefinition::CurveCurveTangency {
                first_contact,
                second_contact,
            } => Some((*first_contact, *second_contact)),
            _ => None,
        })
        .expect("fixture must lower one mixed curve tangency");
    let line_contact_before = accepted_before.contact(first_contact).unwrap().clone();
    let circle_contact_before = accepted_before.contact(second_contact).unwrap().clone();
    let line_parameter = line_contact_before.parameter;
    let circle_parameter = circle_contact_before.parameter;
    assert_eq!(
        accepted_before.scalar(line_parameter).unwrap().unit,
        ScalarUnit::Parameter,
    );
    assert_eq!(
        accepted_before.scalar(circle_parameter).unwrap().unit,
        ScalarUnit::Angle,
    );
    assert_eq!(
        accepted_before.scalar(radius).unwrap().unit,
        ScalarUnit::Length
    );

    let ownership = &coordinator.accepted_materialization().unwrap().ownership;
    let line_parameter_leaf = ownership
        .writable_leaf(IntentNativeWritableLeaf::ScalarValue {
            scalar: line_parameter,
        })
        .expect("line contact parameter must retain one reverse owner");
    let circle_parameter_leaf = ownership
        .writable_leaf(IntentNativeWritableLeaf::ScalarValue {
            scalar: circle_parameter,
        })
        .expect("circle contact parameter must retain one reverse owner");
    let radius_leaf = ownership
        .writable_leaf(IntentNativeWritableLeaf::ScalarValue { scalar: radius })
        .expect("circle radius must retain one reverse owner");
    let instance_before = coordinator.intent().instance().values().clone();
    let history_before = coordinator.intent().undo_len();

    coordinator.begin_point_drag(90_006, line_end).unwrap();
    let preview = coordinator
        .preview_point_drag(90_006, 1, [-2.0, 2.0], OperationControl::unlimited())
        .unwrap()
        .expect("mixed line-circle tangency must have one accepted drag preview");
    assert!(preview.accepted_position.into_iter().all(f64::is_finite));
    let outcome = coordinator.finish_point_drag(90_006, 1).unwrap();

    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
    assert_eq!(coordinator.intent().undo_len(), history_before + 1);
    let expected_line_parameter = 7.0 / 17.0;
    let expected_circle_parameter = 4.0_f64.atan2(1.0);
    let expected_radius = 6.0 / 17.0_f64.sqrt();
    for (leaf, expected, unit) in [
        (
            line_parameter_leaf,
            expected_line_parameter,
            IntentUnit::Dimensionless,
        ),
        (
            circle_parameter_leaf,
            expected_circle_parameter,
            IntentUnit::Dimensionless,
        ),
        (radius_leaf, expected_radius, IntentUnit::Length),
    ] {
        let Some(IntentLiteral::Quantity {
            value,
            unit: actual,
        }) = coordinator.intent().instance().values().get(&leaf)
        else {
            panic!("reverse-projected scalar leaf must remain a quantity");
        };
        assert_eq!(*actual, unit);
        assert!((*value - expected).abs() <= 1.0e-8, "{value} != {expected}");
    }

    let accepted = coordinator.accepted_materialization().unwrap();
    assert!(accepted.validation.hard_residuals_validated);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_some_and(|residual| residual.is_finite() && residual <= 1.0e-9),
        "{:?}",
        accepted.validation,
    );
    let document = accepted
        .session
        .accepted_state_for_current_input()
        .unwrap()
        .document();
    assert!(
        document
            .points()
            .iter()
            .flat_map(|point| point.position)
            .chain(document.scalars().iter().map(|scalar| scalar.value))
            .all(f64::is_finite)
    );
    let assert_point = |actual: [f64; 2], expected: [f64; 2]| {
        assert!((actual[0] - expected[0]).hypot(actual[1] - expected[1]) <= 1.0e-8);
    };
    assert_point(document.point(line_start).unwrap().position, [2.0, 1.0]);
    assert_point(document.point(line_end).unwrap().position, [-2.0, 2.0]);
    assert_point(document.point(circle_center).unwrap().position, [0.0, 0.0]);
    assert!(
        (document.scalar(line_parameter).unwrap().value - expected_line_parameter).abs() <= 1.0e-8
    );
    assert!(
        (document.scalar(circle_parameter).unwrap().value - expected_circle_parameter).abs()
            <= 1.0e-8
    );
    assert!((document.scalar(radius).unwrap().value - expected_radius).abs() <= 1.0e-8);

    let line_contact_after = document.contact(first_contact).unwrap();
    let circle_contact_after = document.contact(second_contact).unwrap();
    for (before, after) in [
        (&line_contact_before, line_contact_after),
        (&circle_contact_before, circle_contact_after),
    ] {
        assert_eq!(after.curve, before.curve);
        assert_eq!(after.domain, before.domain);
        assert_eq!(after.winding, before.winding);
        assert_eq!(after.neighborhood, before.neighborhood);
        assert_eq!(after.tangent_orientation, before.tangent_orientation);
    }
    let line_jet = document.evaluate_contact_jet(first_contact).unwrap();
    let circle_jet = document.evaluate_contact_jet(second_contact).unwrap();
    assert_point(
        [line_jet.position.x, line_jet.position.y],
        [circle_jet.position.x, circle_jet.position.y],
    );
    let tangent_cross = line_jet.first_derivative.x * circle_jet.first_derivative.y
        - line_jet.first_derivative.y * circle_jet.first_derivative.x;
    let tangent_dot = line_jet.first_derivative.x * circle_jet.first_derivative.x
        + line_jet.first_derivative.y * circle_jet.first_derivative.y;
    assert!(tangent_cross.abs() <= 1.0e-8);
    assert!(
        tangent_dot > 0.0,
        "aligned orientation must remain explicit"
    );

    coordinator
        .undo()
        .unwrap()
        .expect("drag must be exactly undoable");
    assert_eq!(coordinator.intent().instance().values(), &instance_before);
    assert_eq!(
        coordinator
            .accepted_materialization()
            .unwrap()
            .session
            .accepted_state_for_current_input()
            .unwrap()
            .document(),
        &accepted_before,
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one underconstrained terminal keeps preview continuity, certification, persistence and Undo together"
)]
fn point_drag_terminal_certifies_the_exact_accepted_underconstrained_preview() {
    let raw = 0x8300_9007;
    let mut coordinator = coordinator(raw);
    let aliases = create(
        &mut coordinator,
        [
            ("base", point("base", [0.0, 0.0])),
            ("elbow", point("elbow", [1.0, 1.0])),
            ("end", point("end", [2.0, 0.0])),
        ],
    );
    let bound_point = |alias| {
        let IntentNativeBinding::Point(point) =
            alias_binding(&coordinator, &aliases, alias, IntentPortRole::Primary)
        else {
            panic!("{alias} must own one point");
        };
        point
    };
    let base = bound_point("base");
    let elbow = bound_point("elbow");
    let end = bound_point("end");

    let fixed = translate(
        &coordinator,
        &relation(
            ConstraintIntent::Lock,
            vec![AuthoringOperand::selected(SelectionItem::Point(base))],
            ResolvedConstraintKind::FixedPoint,
        ),
    )
    .unwrap();
    coordinator.apply_patch(fixed.patch).unwrap();
    for (first, second) in [(base, elbow), (elbow, end)] {
        let dimension = translate(
            &coordinator,
            &AuthoringApplication {
                tool: AuthoringTool::Dimension(AuthoringDimensionKind::PointDistance),
                operands: vec![
                    AuthoringOperand::selected(SelectionItem::Point(first)),
                    AuthoringOperand::selected(SelectionItem::Point(second)),
                ],
                options: AuthoringOptions::default(),
                resolved_constraint: None,
            },
        )
        .unwrap();
        coordinator.apply_patch(dimension.patch).unwrap();
    }

    let accepted_before = coordinator
        .accepted_materialization()
        .unwrap()
        .session
        .accepted_state_for_current_input()
        .unwrap()
        .document()
        .clone();
    let history_before = coordinator.intent().undo_len();
    coordinator.begin_point_drag(90_007, end).unwrap();
    coordinator
        .preview_point_drag(90_007, 1, [0.0, 0.0], OperationControl::unlimited())
        .unwrap()
        .expect("the two-link chain has one accepted terminal preview");
    let preview = coordinator
        .presentation_session()
        .unwrap()
        .accepted_state_for_current_input()
        .unwrap()
        .document()
        .clone();
    assert_ne!(
        preview.point(end).unwrap().position.map(f64::to_bits),
        accepted_before
            .point(end)
            .unwrap()
            .position
            .map(f64::to_bits),
    );

    coordinator.finish_point_drag(90_007, 1).unwrap();
    let accepted = coordinator.accepted_materialization().unwrap();
    let published = accepted
        .session
        .accepted_state_for_current_input()
        .unwrap()
        .document();
    assert_eq!(published, &preview);
    assert!(accepted.validation.hard_residuals_validated);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
    );
    assert_eq!(coordinator.intent().undo_len(), history_before + 1);

    let canonical = coordinator.intent().to_canonical_json().unwrap();
    let restored = ProjectionalIntentCoordinator::restore(
        IntentSession::from_json(&canonical).unwrap(),
        ColdIntentMaterializer::with_default_policy(
            DocumentId(PersistentId::from_u128(raw << 32)),
            1.0,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        restored
            .accepted_materialization()
            .unwrap()
            .session
            .accepted_state_for_current_input()
            .unwrap()
            .document(),
        &preview,
    );

    coordinator.undo().unwrap().expect("drag must be undoable");
    assert_eq!(
        coordinator
            .accepted_materialization()
            .unwrap()
            .session
            .accepted_state_for_current_input()
            .unwrap()
            .document(),
        &accepted_before,
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one contact-rich terminal freezes continuity, locality, validation, metadata, history, Undo and Redo"
)]
fn disconnected_underconstrained_contacts_retain_the_terminal_preview() {
    let mut coordinator = coordinator(0x8300_9008);
    let aliases = create(
        &mut coordinator,
        [
            (
                "circle_a",
                circle(
                    "circle_a",
                    [-1.058_996_046_522_978_6, 3.792_711_470_877_848_4],
                    1.741_155_992_950_786_3,
                ),
            ),
            (
                "circle_b",
                circle(
                    "circle_b",
                    [-4.401_625_972_775_653, 1.152_443_862_041_915_3],
                    1.381_593_248_489_370_5,
                ),
            ),
            (
                "between",
                segment(
                    "between",
                    [-5.493_509_551_454_087, 1.998_960_344_388_117],
                    [-2.218_656_389_778_542_5, 5.091_484_769_059_186],
                )
                .with_field(
                    IntentFieldKey(key("branch_direction")),
                    IntentLiteral::Point([0.737_788_637_523_594_1, 0.675_031_796_540_784]),
                ),
            ),
            (
                "guide",
                segment(
                    "guide",
                    [-1.058_996_046_522_978_6, 1.152_443_862_041_915_3],
                    [-0.910_063_380_846_682_1, -1.745_781_295_964_486_7],
                )
                .with_field(
                    IntentFieldKey(key("branch_direction")),
                    IntentLiteral::Point([0.699_294_155_463_731_2, -0.714_834_025_585_147_4]),
                ),
            ),
            (
                "tangent",
                segment(
                    "tangent",
                    [-5.373_546_124_484_037_5, -1.648_098_981_835_005_4],
                    [2.449_463_235_547_789, 4.260_673_108_107_254],
                )
                .with_field(
                    IntentFieldKey(key("branch_direction")),
                    IntentLiteral::Point([0.769_856_553_302_791_3, 0.638_216_959_455_596_4]),
                ),
            ),
        ],
    );
    let bound_point = |alias, role| {
        let IntentNativeBinding::Point(point) = alias_binding(&coordinator, &aliases, alias, role)
        else {
            panic!("{alias} must own one point");
        };
        point
    };
    let bound_span = |alias| {
        let IntentNativeBinding::CurveSpan(span) =
            alias_binding(&coordinator, &aliases, alias, IntentPortRole::Span)
        else {
            panic!("{alias} must own one span");
        };
        span
    };
    let upper_circle_center = bound_point("circle_a", IntentPortRole::Center);
    let lower_circle_center = bound_point("circle_b", IntentPortRole::Center);
    let between_start = bound_point("between", IntentPortRole::Start);
    let between_end = bound_point("between", IntentPortRole::End);
    let guide_start = bound_point("guide", IntentPortRole::Start);
    let guide_end = bound_point("guide", IntentPortRole::End);
    let circle_a = bound_span("circle_a");
    let circle_b = bound_span("circle_b");
    let tangent = bound_span("tangent");

    for (point, curve, parameter) in [
        (between_start, circle_b, 2.482_107_115_878_143),
        (between_end, circle_a, 2.299_668_758_371_277_4),
    ] {
        let contact = translate(
            &coordinator,
            &relation(
                ConstraintIntent::Coincident,
                vec![
                    AuthoringOperand::selected(SelectionItem::Point(point)),
                    AuthoringOperand::picked(SelectionItem::Curve(curve), Some(parameter)),
                ],
                ResolvedConstraintKind::PointOnCurve,
            ),
        )
        .unwrap();
        coordinator.apply_patch(contact.patch).unwrap();
    }
    for (tool, first, second, resolved) in [
        (
            ConstraintIntent::Horizontal,
            guide_start,
            lower_circle_center,
            ResolvedConstraintKind::HorizontalPoints,
        ),
        (
            ConstraintIntent::Vertical,
            guide_start,
            upper_circle_center,
            ResolvedConstraintKind::VerticalPoints,
        ),
    ] {
        let relation = translate(
            &coordinator,
            &relation(
                tool,
                vec![
                    AuthoringOperand::selected(SelectionItem::Point(first)),
                    AuthoringOperand::selected(SelectionItem::Point(second)),
                ],
                resolved,
            ),
        )
        .unwrap();
        coordinator.apply_patch(relation.patch).unwrap();
    }
    let tangency = translate(
        &coordinator,
        &relation(
            ConstraintIntent::Tangent,
            vec![
                AuthoringOperand::picked(
                    SelectionItem::Curve(circle_a),
                    Some(5.359_277_792_595_967),
                ),
                AuthoringOperand::picked(
                    SelectionItem::Curve(tangent),
                    Some(0.685_664_076_214_954_8),
                ),
            ],
            ResolvedConstraintKind::CurveTangency,
        ),
    )
    .unwrap();
    coordinator.apply_patch(tangency.patch).unwrap();

    let accepted_before = coordinator
        .accepted_materialization()
        .unwrap()
        .session
        .accepted_state_for_current_input()
        .unwrap()
        .document()
        .clone();
    let moved_before = accepted_before.point(between_start).unwrap().position;
    let unrelated_before = accepted_before.point(guide_end).unwrap().position;
    let contacts_before = accepted_before.contacts().to_vec();
    let history_before = coordinator.intent().undo_len();

    coordinator.begin_point_drag(90_008, between_start).unwrap();
    coordinator
        .preview_point_drag(90_008, 1, [-5.0, 2.35], OperationControl::unlimited())
        .unwrap()
        .expect("contact-rich fixture has one accepted terminal preview");
    let preview = coordinator
        .presentation_session()
        .unwrap()
        .accepted_state_for_current_input()
        .unwrap()
        .document()
        .clone();
    let moved_preview = preview.point(between_start).unwrap().position;
    assert!(moved_preview.into_iter().all(f64::is_finite));
    assert_ne!(
        moved_preview.map(f64::to_bits),
        moved_before.map(f64::to_bits)
    );
    assert_eq!(
        preview.point(guide_end).unwrap().position.map(f64::to_bits),
        unrelated_before.map(f64::to_bits),
        "the disconnected guide endpoint must retain drag locality",
    );

    coordinator.finish_point_drag(90_008, 1).unwrap();
    let accepted = coordinator.accepted_materialization().unwrap();
    assert!(accepted.validation.hard_residuals_validated);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
    );
    let published = accepted
        .session
        .accepted_state_for_current_input()
        .unwrap()
        .document();
    assert_eq!(published, &preview);
    assert_eq!(coordinator.intent().undo_len(), history_before + 1);
    assert_eq!(published.contacts().len(), contacts_before.len());
    for (before, after) in contacts_before.iter().zip(published.contacts()) {
        assert_eq!(after.id, before.id);
        assert_eq!(after.curve, before.curve);
        assert_eq!(after.parameter, before.parameter);
        assert_eq!(after.domain, before.domain);
        assert_eq!(after.winding, before.winding);
        assert_eq!(after.neighborhood, before.neighborhood);
        assert_eq!(after.tangent_orientation, before.tangent_orientation);
    }

    coordinator.undo().unwrap().expect("drag must be undoable");
    assert_eq!(
        coordinator
            .accepted_materialization()
            .unwrap()
            .session
            .accepted_state_for_current_input()
            .unwrap()
            .document(),
        &accepted_before,
    );
    coordinator.redo().unwrap().expect("drag must be redoable");
    assert_eq!(
        coordinator
            .accepted_materialization()
            .unwrap()
            .session
            .accepted_state_for_current_input()
            .unwrap()
            .document(),
        &preview,
    );
}

#[test]
fn conflicting_explicit_relation_is_retained_over_prior_accepted_scene() {
    let mut coordinator = coordinator(0x8300_7203);
    create(
        &mut coordinator,
        [("edge", segment("edge", [0.0, 0.0], [4.0, 0.0]))],
    );
    let document = coordinator
        .accepted_materialization()
        .unwrap()
        .session
        .design_document();
    let first = document.points()[0].id;
    let second = document.points()[1].id;
    let edge = span(&coordinator, 0);
    for point in [first, second] {
        let lock = relation(
            ConstraintIntent::Lock,
            vec![AuthoringOperand::selected(SelectionItem::Point(point))],
            ResolvedConstraintKind::FixedPoint,
        );
        let translated = translate(&coordinator, &lock).unwrap();
        assert_eq!(
            coordinator
                .apply_patch(translated.patch)
                .unwrap()
                .disposition,
            IntentPlanDisposition::Accepted
        );
    }
    let accepted_before = coordinator
        .accepted_materialization()
        .unwrap()
        .session
        .design_document()
        .clone();
    let semantic_before = coordinator
        .accepted_materialization()
        .unwrap()
        .ownership
        .semantic;
    let vertical = relation(
        ConstraintIntent::Vertical,
        vec![AuthoringOperand::selected(SelectionItem::Curve(edge))],
        ResolvedConstraintKind::VerticalLine,
    );
    let translated = translate(&coordinator, &vertical).unwrap();
    let node_count = coordinator.intent().graph().nodes().len();
    let outcome = coordinator.apply_patch(translated.patch).unwrap();
    assert_eq!(outcome.disposition, IntentPlanDisposition::RetainedFailed);
    assert_eq!(coordinator.intent().graph().nodes().len(), node_count + 1);
    assert_eq!(
        coordinator
            .accepted_materialization()
            .unwrap()
            .session
            .design_document(),
        &accepted_before
    );
    assert_eq!(
        coordinator
            .accepted_materialization()
            .unwrap()
            .ownership
            .semantic,
        semantic_before
    );
    assert_ne!(coordinator.intent().semantic_identity(), semantic_before);
}

#[test]
fn stale_resolution_and_foreign_ownership_fail_before_planning() {
    let mut first = coordinator(0x8300_7204);
    create(
        &mut first,
        [("edge", segment("edge", [0.0, 0.0], [4.0, 0.0]))],
    );
    let edge = span(&first, 0);
    let stale = relation(
        ConstraintIntent::Horizontal,
        vec![AuthoringOperand::selected(SelectionItem::Curve(edge))],
        ResolvedConstraintKind::VerticalLine,
    );
    assert_eq!(
        translate(&first, &stale).unwrap_err(),
        ProjectionalAuthoringError::StaleAuthoringResolution
    );

    let mut foreign = coordinator(0x8300_7205);
    create(
        &mut foreign,
        [("other", segment("other", [0.0, 1.0], [4.0, 1.0]))],
    );
    let valid = relation(
        ConstraintIntent::Horizontal,
        vec![AuthoringOperand::selected(SelectionItem::Curve(edge))],
        ResolvedConstraintKind::HorizontalLine,
    );
    let accepted = first.accepted_materialization().unwrap();
    assert_eq!(
        projectional_application_patch(
            first.intent().identity(),
            first.intent(),
            &foreign.accepted_materialization().unwrap().ownership,
            accepted.session.design_document(),
            accepted
                .session
                .accepted_state_for_current_input()
                .unwrap()
                .document(),
            &valid,
        )
        .unwrap_err(),
        ProjectionalAuthoringError::StaleOwnership
    );
}

#[test]
fn projectional_editor_routes_authoring_through_its_sole_coordinator() {
    let mut coordinator = coordinator(0x8300_7206);
    create(
        &mut coordinator,
        [("edge", segment("edge", [0.0, 0.0], [4.0, 0.0]))],
    );
    let edge = span(&coordinator, 0);
    let mut editor = ProjectionalEditorSession::new(coordinator);
    editor.set_selection([SelectionItem::Curve(edge)]);
    let history_before = editor
        .coordinator()
        .intent()
        .history_projection()
        .applied
        .len();

    let outcome = editor
        .apply_authoring_application(&relation(
            ConstraintIntent::Horizontal,
            vec![AuthoringOperand::selected(SelectionItem::Curve(edge))],
            ResolvedConstraintKind::HorizontalLine,
        ))
        .unwrap();
    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
    assert_eq!(
        editor
            .coordinator()
            .intent()
            .history_projection()
            .applied
            .len(),
        history_before + 1
    );
    assert!(editor.editor().selection().is_empty());
    assert_eq!(
        editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .design_document()
            .constraints()
            .len(),
        1
    );

    assert!(editor.undo().unwrap().is_some());
    assert!(
        editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .design_document()
            .constraints()
            .is_empty()
    );
}

#[test]
fn projectional_editor_authenticates_inspector_edits_into_the_same_history() {
    let mut coordinator = coordinator(0x8300_7207);
    create(&mut coordinator, [("point", point("point", [1.0, 2.0]))]);
    let node = *coordinator.intent().graph().nodes().keys().next().unwrap();
    let mut editor = ProjectionalEditorSession::new(coordinator);
    assert!(editor.set_selected_declaration(Some(node)));
    let projection = editor.workbench_projection();
    let inspector = editor.selected_inspector(&projection).unwrap();
    let leaf = inspector
        .fields
        .iter()
        .find_map(|field| match field {
            IntentInspectorField::Instance { leaf, .. } if leaf.field == LeafField::X => {
                Some(*leaf)
            }
            _ => None,
        })
        .unwrap();
    let history_before = editor
        .coordinator()
        .intent()
        .history_projection()
        .applied
        .len();

    let outcome = editor
        .edit_inspector(
            &inspector,
            &IntentInspectorEditTarget::Instance { leaf },
            IntentInspectorEditValue::Literal {
                literal: coordinate(9.0),
            },
        )
        .unwrap();
    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
    assert_eq!(
        editor
            .coordinator()
            .intent()
            .history_projection()
            .applied
            .len(),
        history_before + 1
    );
    assert_eq!(
        editor
            .coordinator()
            .presentation_session()
            .unwrap()
            .design_document()
            .points()[0]
            .position
            .map(f64::to_bits),
        [9.0, 2.0].map(f64::to_bits)
    );

    assert!(editor.undo().unwrap().is_some());
    assert_eq!(
        editor
            .coordinator()
            .presentation_session()
            .unwrap()
            .design_document()
            .points()[0]
            .position
            .map(f64::to_bits),
        [1.0, 2.0].map(f64::to_bits)
    );
}
