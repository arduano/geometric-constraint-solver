// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    AuthoringApplication, AuthoringOperand, AuthoringOptions, AuthoringTool,
    ColdIntentMaterializer, ConstraintIntent, DimensionKind as AuthoringDimensionKind,
    IntentInspectorEditTarget, IntentInspectorEditValue, IntentInspectorField, IntentNativeBinding,
    ProjectionalAuthoringError, ProjectionalEditorSession, ProjectionalIntentCoordinator,
    ResolvedConstraintKind, SelectionItem, projectional_application_patch,
};
use geosolve_sketch::{
    CurveSpan, DocumentConstraintDefinition, DocumentDimensionDefinition, DocumentId, PersistentId,
};
use geosolve_sketch_intent::{
    ConstraintKind, DimensionKind as IntentDimensionKind, GeometryRecipeKind, IntentAliasMap,
    IntentKey, IntentLiteral, IntentNodeDraft, IntentNodeKind, IntentPatch, IntentPatchOperation,
    IntentPatchPolicy, IntentPlanDisposition, IntentPortRole, IntentPortSelector, IntentSessionId,
    IntentUnit, LeafField,
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
        draft
            .fields
            .contains_key(&geosolve_sketch_intent::IntentFieldKey(key(
                "contact_domain"
            )))
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
