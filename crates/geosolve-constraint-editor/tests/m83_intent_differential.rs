// SPDX-License-Identifier: GPL-3.0-or-later

use std::cell::RefCell;

use geosolve_constraint_editor::{
    ColdIntentMaterialization, ColdIntentMaterializer, FeatureAuthoringCandidate,
    FeatureAuthoringOptions, FeatureAuthoringOutcome, FeatureAuthoringState, FeatureAuthoringTool,
    SelectionItem, projectional_fillet_patch,
};
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DocumentBSplineForm, DocumentConstraintDefinition,
    DocumentDirectedProfileOffsetCurve, DocumentElementId, DocumentExternalBindingId,
    DocumentExternalPointRef, DocumentId, DocumentLineSide, DocumentOffsetTraversal,
    DocumentParameterId, DocumentParameterKind, DocumentParameterTarget,
    DocumentProfileOffsetChain, DocumentProfileOffsetEdgePair, DocumentProfileOffsetOperand,
    DocumentProfileOffsetTerminalPolicy, DocumentSolveRequest, ExternalFeatureKindV1,
    ExternalSnapshotDigest, ExternalSnapshotEntry, ExternalSnapshotFeatureV1,
    ExternalSnapshotResourcesV1, ExternalSnapshotSet, OperationControl, OperationOutcome,
    ParameterBatch, ParameterBatchEntry, ParameterValue, PersistentId,
    RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchAcceptedDocumentState,
    SketchDocument, SketchExternalReferenceState, SketchHardValidity, SketchParameterState,
    SolverConfig,
};
use geosolve_sketch_features::{
    ComputedEdgeGeometry, ComputedEvaluationAllocator, ComputedFeatureAuthoringSnapshot,
    ComputedFeatureDefinition, ComputedFeatureDocument, ComputedFeatureEvaluationPolicy,
    ComputedFeatureEvaluationSnapshot, ComputedFeatureEvaluationState, ComputedFeatureSnapshot,
    ComputedFilletParent, NativeCurveSpanSource,
};
use geosolve_sketch_intent::{
    AggregateKind, ConstraintKind, DimensionKind, ExternalInputRevision, ExternalIntentKind,
    GeometryRecipeKind, InputRole, InputSlot, IntentEvaluation, IntentExternalInputs,
    IntentFieldKey, IntentKey, IntentLiteral, IntentNativeReservationKind, IntentNodeDraft,
    IntentNodeKind, IntentPatch, IntentPatchOperation, IntentPatchPolicy, IntentPortRole,
    IntentPortSelector, IntentSession, IntentSessionId, IntentUnit, LeafField, ParameterIntentKind,
    PatchPortRef,
};

fn key(value: &str) -> IntentKey {
    IntentKey::new(value).unwrap()
}

const fn selector(role: IntentPortRole, index: u16) -> IntentPortSelector {
    IntentPortSelector::Node { role, index }
}

const fn child_selector(ordinal: u16, role: IntentPortRole) -> IntentPortSelector {
    IntentPortSelector::InitialChild {
        ordinal,
        role,
        index: 0,
    }
}

fn quantity(value: f64, unit: IntentUnit) -> IntentLiteral {
    IntentLiteral::Quantity { value, unit }
}

fn coordinate(value: f64) -> IntentLiteral {
    quantity(value, IntentUnit::Length)
}

fn parameter(value: f64) -> IntentLiteral {
    quantity(value, IntentUnit::Dimensionless)
}

fn alias(node: &str, role: IntentPortRole, index: u16) -> PatchPortRef {
    PatchPortRef::Alias {
        node: key(node),
        selector: selector(role, index),
    }
}

fn create(alias: &str, draft: IntentNodeDraft) -> IntentPatchOperation {
    IntentPatchOperation::CreateNode {
        alias: key(alias),
        draft: Box::new(draft),
        cell: None,
    }
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

fn sketch_point(name: &str, position: [f64; 2]) -> IntentNodeDraft {
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

fn segment_between(name: &str, start: &str, end: &str, direction: [f64; 2]) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        },
        key(name),
    )
    .with_input(
        InputSlot::new(InputRole::Point, 0),
        alias(start, IntentPortRole::Primary, 0),
    )
    .with_input(
        InputSlot::new(InputRole::Point, 1),
        alias(end, IntentPortRole::Primary, 0),
    )
    .with_field(
        IntentFieldKey(key("branch_direction")),
        IntentLiteral::Point(direction),
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

fn rational_conic(
    name: &str,
    start: [f64; 2],
    weighted_middle: [f64; 2],
    weight: f64,
    end: [f64; 2],
) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::RationalQuadraticConic,
        },
        key(name),
    )
    .with_field(
        IntentFieldKey(key("weighted_middle")),
        IntentLiteral::Point(weighted_middle),
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
        selector(IntentPortRole::Target, 0),
        LeafField::Weight,
        parameter(weight),
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

fn quadratic_bezier(name: &str, controls: [[f64; 2]; 3]) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::QuadraticBezier,
        },
        key(name),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Start, 0),
        LeafField::X,
        coordinate(controls[0][0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Start, 0),
        LeafField::Y,
        coordinate(controls[0][1]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Control, 0),
        LeafField::X,
        coordinate(controls[1][0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Control, 0),
        LeafField::Y,
        coordinate(controls[1][1]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::End, 0),
        LeafField::X,
        coordinate(controls[2][0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::End, 0),
        LeafField::Y,
        coordinate(controls[2][1]),
    )
}

fn open_nurbs(name: &str, controls: [[f64; 2]; 4], weights: [f64; 4]) -> IntentNodeDraft {
    let mut draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::OpenControlNurbs,
        },
        key(name),
    )
    .with_dynamic_children(4)
    .with_field(IntentFieldKey(key("degree")), IntentLiteral::Natural(3))
    .with_field(
        IntentFieldKey(key("gauge_index")),
        IntentLiteral::Natural(0),
    );
    for (ordinal, (control, weight)) in controls.into_iter().zip(weights).enumerate() {
        let ordinal = u16::try_from(ordinal).unwrap();
        draft = draft
            .with_instance_leaf(
                child_selector(ordinal, IntentPortRole::Control),
                LeafField::X,
                coordinate(control[0]),
            )
            .with_instance_leaf(
                child_selector(ordinal, IntentPortRole::Control),
                LeafField::Y,
                coordinate(control[1]),
            )
            .with_instance_leaf(
                child_selector(ordinal, IntentPortRole::Target),
                LeafField::Weight,
                parameter(weight),
            );
    }
    draft
}

fn activation_parameter(name: &str) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Parameter {
            parameter: ParameterIntentKind::Parameter,
        },
        key(name),
    )
    .with_field(
        IntentFieldKey(key("kind")),
        IntentLiteral::Enum(key("activation")),
    )
}

struct IntentFixture {
    session: IntentSession,
    materializer: ColdIntentMaterializer,
    output: ColdIntentMaterialization,
}

fn intent_fixture(raw: u128, operations: Vec<IntentPatchOperation>) -> IntentFixture {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(raw)).unwrap();
    let materializer = ColdIntentMaterializer::with_default_policy(
        DocumentId(PersistentId::from_u128(raw << 32)),
        1.0,
    )
    .unwrap();
    let captured = RefCell::new(None);
    let plan = session
        .plan_patch(
            IntentPatch::new(
                session.identity(),
                IntentPatchPolicy::RequireAccepted,
                operations,
            ),
            |candidate| {
                let output = materializer.materialize(candidate).unwrap();
                let evidence = output.evidence.clone();
                captured.replace(Some(output));
                IntentEvaluation::Accepted { evidence }
            },
        )
        .unwrap();
    session.commit_plan(plan).unwrap();
    let output = captured.into_inner().unwrap();
    IntentFixture {
        session,
        materializer,
        output,
    }
}

fn native_session(document: SketchDocument) -> RetainedSketchDocumentSession {
    RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap()
}

fn accepted(session: &RetainedSketchDocumentSession) -> &SketchAcceptedDocumentState {
    session
        .accepted_state_for_current_input()
        .expect("fixture must have current accepted state")
}

fn assert_independently_valid(label: &str, accepted: &SketchAcceptedDocumentState) {
    let diagnostics = accepted.diagnostics();
    let solve = diagnostics.solve.expect("accepted solve diagnostics");
    assert!(solve.accepted, "{label}: {solve:#?}");
    assert_eq!(solve.hard_validity, SketchHardValidity::Valid, "{label}");
    assert!(solve.hard_residuals_validated, "{label}");
    assert!(
        solve
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1.0e-9),
        "{label}: {solve:#?}"
    );
    let rank = diagnostics.rank.expect("accepted rank diagnostics");
    assert!(rank.numerical_valid, "{label}: {rank:#?}");
    assert!(rank.numerical_rank.is_some(), "{label}: {rank:#?}");
    assert!(
        diagnostics
            .mobility
            .expect("accepted mobility diagnostics")
            .equality_degrees_of_freedom
            .is_some(),
        "{label}"
    );
    let document = accepted.document();
    assert!(
        document
            .points()
            .iter()
            .flat_map(|point| point.position)
            .all(f64::is_finite)
    );
    assert!(
        document
            .scalars()
            .iter()
            .all(|scalar| scalar.value.is_finite())
    );
    for curve in document.curves() {
        for span in document.curve_spans(curve.id).unwrap() {
            for parameter in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let jet = document.evaluate_curve_jet(span, parameter).unwrap();
                assert!(
                    [
                        jet.position.x,
                        jet.position.y,
                        jet.first_derivative.x,
                        jet.first_derivative.y,
                        jet.second_derivative.x,
                        jet.second_derivative.y,
                        jet.third_derivative.x,
                        jet.third_derivative.y,
                    ]
                    .into_iter()
                    .all(f64::is_finite),
                    "{label}: non-finite {span:?} jet at {parameter}"
                );
            }
        }
    }
}

fn assert_close(label: &str, actual: f64, expected: f64) {
    let scale = actual.abs().max(expected.abs()).max(1.0);
    assert!(
        (actual - expected).abs() <= 1.0e-10 * scale,
        "{label}: expected {expected:.17e}, got {actual:.17e}"
    );
}

#[allow(
    clippy::match_same_arms,
    reason = "line branch directions and conic weighted middles are intentionally named separately in differential failures"
)]
fn assert_curve_branch_parity(label: &str, intent: &CurveDefinition, flat: &CurveDefinition) {
    match (intent, flat) {
        (
            CurveDefinition::Line {
                branch_direction: first,
                ..
            },
            CurveDefinition::Line {
                branch_direction: second,
                ..
            },
        ) => {
            assert_close(label, first[0], second[0]);
            assert_close(label, first[1], second[1]);
        }
        (CurveDefinition::Circle { .. }, CurveDefinition::Circle { .. })
        | (CurveDefinition::QuadraticBezier { .. }, CurveDefinition::QuadraticBezier { .. }) => {}
        (
            CurveDefinition::RationalQuadraticConic {
                weighted_middle: first,
                ..
            },
            CurveDefinition::RationalQuadraticConic {
                weighted_middle: second,
                ..
            },
        ) => {
            assert_close(label, first[0], second[0]);
            assert_close(label, first[1], second[1]);
        }
        (
            CurveDefinition::Nurbs {
                form: first_form,
                degree: first_degree,
                span_ids: first_spans,
                ..
            },
            CurveDefinition::Nurbs {
                form: second_form,
                degree: second_degree,
                span_ids: second_spans,
                ..
            },
        ) => {
            assert_eq!(first_form, second_form, "{label}");
            assert_eq!(first_degree, second_degree, "{label}");
            assert_eq!(first_spans.len(), second_spans.len(), "{label}");
        }
        (first, second) => panic!("{label}: curve families differ: {first:?} / {second:?}"),
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one shared differential oracle keeps solve, inventory, curve-jet and cold-replay parity atomic"
)]
fn assert_flat_parity(label: &str, fixture: &IntentFixture, flat: &RetainedSketchDocumentSession) {
    let intent = accepted(&fixture.output.session);
    let flat = accepted(flat);
    assert_independently_valid(&format!("{label} intent"), intent);
    assert_independently_valid(&format!("{label} flat"), flat);

    let intent_diagnostics = intent.diagnostics();
    let flat_diagnostics = flat.diagnostics();
    assert_eq!(intent_diagnostics.rank, flat_diagnostics.rank, "{label}");
    assert_eq!(
        intent_diagnostics.mobility, flat_diagnostics.mobility,
        "{label}"
    );
    let intent_solve = intent_diagnostics.solve.unwrap();
    let flat_solve = flat_diagnostics.solve.unwrap();
    assert_eq!(intent_solve.accepted, flat_solve.accepted, "{label}");
    assert_eq!(
        intent_solve.hard_validity, flat_solve.hard_validity,
        "{label}"
    );
    assert_eq!(intent_solve.termination, flat_solve.termination, "{label}");

    let intent_document = intent.document();
    let flat_document = flat.document();
    assert_eq!(
        intent_document.points().len(),
        flat_document.points().len(),
        "{label}"
    );
    assert_eq!(
        intent_document.scalars().len(),
        flat_document.scalars().len(),
        "{label}"
    );
    assert_eq!(
        intent_document.curves().len(),
        flat_document.curves().len(),
        "{label}"
    );
    assert_eq!(
        intent_document.contacts().len(),
        flat_document.contacts().len(),
        "{label}"
    );
    assert_eq!(
        intent_document.constraints().len(),
        flat_document.constraints().len(),
        "{label}"
    );
    assert_eq!(
        intent_document.dimensions().len(),
        flat_document.dimensions().len(),
        "{label}"
    );
    let source_signature = |diagnostics: &geosolve_sketch::SketchDiagnosticSnapshot| {
        diagnostics
            .sources
            .iter()
            .map(|source| {
                (
                    source.label.clone(),
                    source.inactivity,
                    source.active_row_count,
                    source.evaluated_row_count,
                    source.failed_row_count,
                    source.conflict_candidate,
                    source.fully_redundant,
                    source.contains_redundant_rows,
                    source.singular,
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        source_signature(&intent_diagnostics),
        source_signature(&flat_diagnostics),
        "{label} source diagnostics"
    );
    for (index, (intent_curve, flat_curve)) in intent_document
        .curves()
        .iter()
        .zip(flat_document.curves())
        .enumerate()
    {
        let curve_label = format!("{label} curve {index}");
        assert_curve_branch_parity(
            &curve_label,
            &intent_curve.definition,
            &flat_curve.definition,
        );
        let intent_spans = intent_document.curve_spans(intent_curve.id).unwrap();
        let flat_spans = flat_document.curve_spans(flat_curve.id).unwrap();
        assert_eq!(intent_spans.len(), flat_spans.len(), "{curve_label}");
        for (span_index, (intent_span, flat_span)) in
            intent_spans.into_iter().zip(flat_spans).enumerate()
        {
            for parameter in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let intent_jet = intent_document
                    .evaluate_curve_jet(intent_span, parameter)
                    .unwrap();
                let flat_jet = flat_document
                    .evaluate_curve_jet(flat_span, parameter)
                    .unwrap();
                for (field, actual, expected) in [
                    ("position.x", intent_jet.position.x, flat_jet.position.x),
                    ("position.y", intent_jet.position.y, flat_jet.position.y),
                    (
                        "first.x",
                        intent_jet.first_derivative.x,
                        flat_jet.first_derivative.x,
                    ),
                    (
                        "first.y",
                        intent_jet.first_derivative.y,
                        flat_jet.first_derivative.y,
                    ),
                    (
                        "second.x",
                        intent_jet.second_derivative.x,
                        flat_jet.second_derivative.x,
                    ),
                    (
                        "second.y",
                        intent_jet.second_derivative.y,
                        flat_jet.second_derivative.y,
                    ),
                ] {
                    assert_close(
                        &format!("{curve_label} span {span_index} t={parameter} {field}"),
                        actual,
                        expected,
                    );
                }
            }
        }
    }

    assert!(
        fixture.output.validation.hard_residuals_validated,
        "{label}"
    );
    assert!(
        fixture
            .output
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1.0e-9),
        "{label}"
    );

    let canonical = fixture.session.to_canonical_json().unwrap();
    let restored = IntentSession::from_json(&canonical).unwrap();
    assert_eq!(restored.to_canonical_json().unwrap(), canonical, "{label}");
    let replayed = fixture
        .materializer
        .materialize_accepted_authority(restored.accepted().unwrap())
        .unwrap();
    assert_eq!(replayed.evidence, fixture.output.evidence, "{label}");
    assert_eq!(replayed.ownership, fixture.output.ownership, "{label}");
    assert_eq!(replayed.validation, fixture.output.validation, "{label}");
    assert_eq!(
        accepted(&replayed.session).diagnostics().rank,
        intent_diagnostics.rank,
        "{label} cold replay rank"
    );
    assert_eq!(
        accepted(&replayed.session).diagnostics().mobility,
        intent_diagnostics.mobility,
        "{label} cold replay mobility"
    );
}

fn complete<T: std::fmt::Debug>(outcome: OperationOutcome<T>) -> T {
    match outcome {
        OperationOutcome::Completed { value, .. } => value,
        other => panic!("expected completed operation, got {other:?}"),
    }
}

fn fillet_candidate(
    session: &RetainedSketchDocumentSession,
    radius: f64,
) -> FeatureAuthoringCandidate {
    let snapshot = ComputedFeatureAuthoringSnapshot::capture(session).unwrap();
    let document = snapshot.sketch_document();
    let picks = document
        .curves()
        .iter()
        .map(|curve| {
            let parameter = match curve.label.as_str() {
                "first" => 0.75,
                "second" => 0.25,
                label => panic!("unexpected Fillet source label {label}"),
            };
            (
                SelectionItem::Curve(CurveSpan::line(curve.id)),
                Some(parameter),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(picks.len(), 2);
    let mut state = FeatureAuthoringState::default();
    assert!(matches!(
        state.activate(&snapshot, document, FeatureAuthoringTool::Fillet, &[]),
        FeatureAuthoringOutcome::ModeEntered(_) | FeatureAuthoringOutcome::Collecting { .. }
    ));
    assert!(matches!(
        state.set_options(
            &snapshot,
            FeatureAuthoringOptions {
                fillet_radius: Some(radius),
                ..FeatureAuthoringOptions::default()
            },
        ),
        FeatureAuthoringOutcome::ModeEntered(_) | FeatureAuthoringOutcome::Collecting { .. }
    ));
    match state.pick_items(&snapshot, document, &picks) {
        FeatureAuthoringOutcome::PreviewRequested { candidate, .. }
        | FeatureAuthoringOutcome::Apply(candidate) => candidate,
        other => panic!("expected complete Fillet candidate, got {other:?}"),
    }
}

fn evaluate_features(
    session: &RetainedSketchDocumentSession,
    features: &ComputedFeatureDocument,
) -> ComputedFeatureSnapshot {
    let mut allocator = ComputedEvaluationAllocator::default();
    let snapshot = ComputedFeatureEvaluationSnapshot::capture(
        session,
        features,
        ComputedFeatureEvaluationPolicy::default(),
    )
    .unwrap();
    complete(
        snapshot
            .prepare(&mut allocator)
            .unwrap()
            .execute(OperationControl::unlimited())
            .unwrap(),
    )
}

fn source_ordinal(document: &SketchDocument, source: NativeCurveSpanSource) -> (usize, u32) {
    let ordinal = document
        .curves()
        .iter()
        .position(|curve| curve.id == source.span.curve)
        .expect("computed source must belong to the accepted sketch");
    (ordinal, source.span.segment)
}

fn assert_fillet_parent_parity(
    label: &str,
    intent_document: &SketchDocument,
    intent: ComputedFilletParent,
    flat_document: &SketchDocument,
    flat: ComputedFilletParent,
) {
    assert_eq!(
        source_ordinal(intent_document, intent.source),
        source_ordinal(flat_document, flat.source),
        "{label} source"
    );
    assert_close(
        &format!("{label} picked parameter"),
        intent.picked_parameter,
        flat.picked_parameter,
    );
    assert_eq!(intent.winding, flat.winding, "{label} winding");
    assert_eq!(
        intent.neighborhood, flat.neighborhood,
        "{label} neighborhood"
    );
    assert_eq!(intent.normal_side, flat.normal_side, "{label} normal side");
    assert_eq!(
        intent.retained_endpoint, flat.retained_endpoint,
        "{label} retained endpoint"
    );
    assert_eq!(
        intent.periodic_anchor, flat.periodic_anchor,
        "{label} periodic anchor"
    );
}

#[allow(
    clippy::too_many_lines,
    reason = "computed Fillet parity deliberately audits every generated and discarded geometry field"
)]
fn assert_computed_fillet_parity(
    intent_document: &SketchDocument,
    intent: &ComputedFeatureSnapshot,
    flat_document: &SketchDocument,
    flat: &ComputedFeatureSnapshot,
) {
    assert_eq!(
        intent.feature_evaluations().len(),
        flat.feature_evaluations().len()
    );
    assert!(
        intent
            .feature_evaluations()
            .iter()
            .all(|evaluation| matches!(
                evaluation.state,
                ComputedFeatureEvaluationState::Current { .. }
            ))
    );
    assert!(flat.feature_evaluations().iter().all(|evaluation| matches!(
        evaluation.state,
        ComputedFeatureEvaluationState::Current { .. }
    )));
    assert_eq!(intent.edges().len(), flat.edges().len());
    for (index, (intent_edge, flat_edge)) in intent.edges().iter().zip(flat.edges()).enumerate() {
        assert_eq!(intent_edge.role, flat_edge.role, "edge {index} role");
        match (&intent_edge.geometry, &flat_edge.geometry) {
            (
                ComputedEdgeGeometry::NativeSourceFragment {
                    source: intent_source,
                    interval: intent_interval,
                },
                ComputedEdgeGeometry::NativeSourceFragment {
                    source: flat_source,
                    interval: flat_interval,
                },
            ) => {
                assert_eq!(
                    source_ordinal(intent_document, *intent_source),
                    source_ordinal(flat_document, *flat_source),
                    "edge {index} source"
                );
                assert_close(
                    &format!("edge {index} interval start"),
                    intent_interval.start,
                    flat_interval.start,
                );
                assert_close(
                    &format!("edge {index} interval end"),
                    intent_interval.end,
                    flat_interval.end,
                );
            }
            (
                ComputedEdgeGeometry::CircularArc(intent_arc),
                ComputedEdgeGeometry::CircularArc(flat_arc),
            ) => {
                for (field, actual, expected) in [
                    ("center.x", intent_arc.center[0], flat_arc.center[0]),
                    ("center.y", intent_arc.center[1], flat_arc.center[1]),
                    ("radius", intent_arc.radius, flat_arc.radius),
                    ("start angle", intent_arc.start_angle, flat_arc.start_angle),
                    ("end angle", intent_arc.end_angle, flat_arc.end_angle),
                ] {
                    assert_close(&format!("edge {index} arc {field}"), actual, expected);
                }
                assert_eq!(intent_arc.sweep, flat_arc.sweep, "edge {index} sweep");
                assert_eq!(
                    intent_arc.tangent_orientations, flat_arc.tangent_orientations,
                    "edge {index} tangent orientations"
                );
                for (contact_index, (intent_contact, flat_contact)) in intent_arc
                    .contacts
                    .iter()
                    .zip(flat_arc.contacts.iter())
                    .enumerate()
                {
                    assert_eq!(
                        source_ordinal(intent_document, intent_contact.source),
                        source_ordinal(flat_document, flat_contact.source),
                        "edge {index} contact {contact_index} source"
                    );
                    assert_eq!(
                        intent_contact.winding, flat_contact.winding,
                        "edge {index} contact {contact_index} winding"
                    );
                    for (field, actual, expected) in [
                        (
                            "parameter",
                            intent_contact.parameter,
                            flat_contact.parameter,
                        ),
                        (
                            "total parameter",
                            intent_contact.total_parameter,
                            flat_contact.total_parameter,
                        ),
                        (
                            "position.x",
                            intent_contact.position[0],
                            flat_contact.position[0],
                        ),
                        (
                            "position.y",
                            intent_contact.position[1],
                            flat_contact.position[1],
                        ),
                    ] {
                        assert_close(
                            &format!("edge {index} contact {contact_index} {field}"),
                            actual,
                            expected,
                        );
                    }
                }
            }
            (intent_geometry, flat_geometry) => panic!(
                "edge {index} geometry families differ: {intent_geometry:?} / {flat_geometry:?}"
            ),
        }
    }

    assert_eq!(
        intent.construction_fragments().len(),
        flat.construction_fragments().len()
    );
    for (index, (intent_fragment, flat_fragment)) in intent
        .construction_fragments()
        .iter()
        .zip(flat.construction_fragments())
        .enumerate()
    {
        assert_eq!(
            source_ordinal(intent_document, intent_fragment.source),
            source_ordinal(flat_document, flat_fragment.source),
            "construction fragment {index} source"
        );
        assert_eq!(
            intent_fragment.source_role, flat_fragment.source_role,
            "construction fragment {index} source role"
        );
        assert_eq!(
            intent_fragment.provenance.endpoint, flat_fragment.provenance.endpoint,
            "construction fragment {index} endpoint"
        );
        for (field, actual, expected) in [
            (
                "start",
                intent_fragment.interval.start,
                flat_fragment.interval.start,
            ),
            (
                "end",
                intent_fragment.interval.end,
                flat_fragment.interval.end,
            ),
            (
                "base start",
                intent_fragment.provenance.base_interval.start,
                flat_fragment.provenance.base_interval.start,
            ),
            (
                "base end",
                intent_fragment.provenance.base_interval.end,
                flat_fragment.provenance.base_interval.end,
            ),
        ] {
            assert_close(
                &format!("construction fragment {index} {field}"),
                actual,
                expected,
            );
        }
    }
}

#[test]
fn affine_and_circular_intent_match_direct_flat_native_authoring() {
    let affine_raw = 0x830d_0001_u128;
    let horizontal = IntentNodeDraft::new(
        IntentNodeKind::Constraint {
            constraint: ConstraintKind::Horizontal,
        },
        key("horizontal"),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 0),
        alias("line", IntentPortRole::Span, 0),
    );
    let affine = intent_fixture(
        affine_raw,
        vec![
            create("line", segment("line", [-2.0, 1.0], [3.0, 1.0])),
            create("horizontal", horizontal),
        ],
    );
    let mut flat_document = SketchDocument::with_id(
        1.0,
        DocumentId(PersistentId::from_u128((affine_raw + 0x1000) << 32)),
    )
    .unwrap();
    let start = flat_document.add_point("start", [-2.0, 1.0]).unwrap();
    let end = flat_document.add_point("end", [3.0, 1.0]).unwrap();
    let line = flat_document
        .add_curve(
            "line",
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    flat_document
        .add_constraint(
            "horizontal",
            DocumentConstraintDefinition::Horizontal {
                line: CurveSpan::line(line),
            },
        )
        .unwrap();
    assert_flat_parity("affine horizontal", &affine, &native_session(flat_document));

    let circular_raw = 0x830d_0002_u128;
    let circular = intent_fixture(
        circular_raw,
        vec![create("circle", circle("circle", [1.5, -2.0], 3.25))],
    );
    let mut flat_document = SketchDocument::with_id(
        1.0,
        DocumentId(PersistentId::from_u128((circular_raw + 0x1000) << 32)),
    )
    .unwrap();
    let center = flat_document.add_point("center", [1.5, -2.0]).unwrap();
    let radius = flat_document
        .add_scalar("radius", 3.25, ScalarUnit::Length, ScalarDomain::Positive)
        .unwrap();
    flat_document
        .add_curve("circle", CurveDefinition::Circle { center, radius })
        .unwrap();
    assert_flat_parity("circle", &circular, &native_session(flat_document));
}

#[test]
fn conic_bezier_and_nurbs_intent_match_direct_flat_native_authoring() {
    let conic_raw = 0x830d_0010_u128;
    let conic = intent_fixture(
        conic_raw,
        vec![create(
            "conic",
            rational_conic("conic", [-2.0, 0.0], [0.5, 2.0], 0.75, [3.0, 1.0]),
        )],
    );
    let mut flat_document = SketchDocument::with_id(
        1.0,
        DocumentId(PersistentId::from_u128((conic_raw + 0x1000) << 32)),
    )
    .unwrap();
    let start = flat_document.add_point("start", [-2.0, 0.0]).unwrap();
    let end = flat_document.add_point("end", [3.0, 1.0]).unwrap();
    let middle_weight = flat_document
        .add_scalar(
            "middle weight",
            0.75,
            ScalarUnit::Parameter,
            ScalarDomain::Bounded {
                lower: geosolve_sketch::MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
                upper: f64::MAX,
            },
        )
        .unwrap();
    flat_document
        .add_curve(
            "conic",
            CurveDefinition::RationalQuadraticConic {
                start,
                weighted_middle: [0.5, 2.0],
                middle_weight,
                end,
            },
        )
        .unwrap();
    assert_flat_parity("rational conic", &conic, &native_session(flat_document));

    let bezier_raw = 0x830d_0011_u128;
    let controls = [[-1.0, 0.0], [0.5, 2.0], [3.0, -0.5]];
    let bezier = intent_fixture(
        bezier_raw,
        vec![create("bezier", quadratic_bezier("bezier", controls))],
    );
    let mut flat_document = SketchDocument::with_id(
        1.0,
        DocumentId(PersistentId::from_u128((bezier_raw + 0x1000) << 32)),
    )
    .unwrap();
    let controls = controls.map(|position| flat_document.add_point("control", position).unwrap());
    flat_document
        .add_curve("bezier", CurveDefinition::QuadraticBezier { controls })
        .unwrap();
    assert_flat_parity("quadratic Bezier", &bezier, &native_session(flat_document));

    let nurbs_raw = 0x830d_0012_u128;
    let controls = [[0.0, 0.0], [1.0, 2.0], [3.0, 2.0], [4.0, 0.0]];
    let weights = [1.0, 0.8, 1.2, 0.9];
    let nurbs = intent_fixture(
        nurbs_raw,
        vec![create("nurbs", open_nurbs("nurbs", controls, weights))],
    );
    let mut flat_document = SketchDocument::with_id(
        1.0,
        DocumentId(PersistentId::from_u128((nurbs_raw + 0x1000) << 32)),
    )
    .unwrap();
    let controls = controls.map(|position| flat_document.add_point("control", position).unwrap());
    let weights = weights.map(|value| {
        flat_document
            .add_scalar(
                "weight",
                value,
                ScalarUnit::Parameter,
                ScalarDomain::Positive,
            )
            .unwrap()
    });
    flat_document
        .add_curve(
            "nurbs",
            CurveDefinition::Nurbs {
                form: DocumentBSplineForm::Clamped,
                degree: 3,
                controls: controls.to_vec(),
                weights: weights.to_vec(),
                gauge_weight: weights[0],
                knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
                span_ids: vec![1],
                next_span_id: 2,
            },
        )
        .unwrap();
    assert_flat_parity("open NURBS", &nurbs, &native_session(flat_document));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the Profile Offset differential keeps its complete explicit open-chain branch beside the native operand"
)]
fn profile_offset_intent_matches_direct_flat_native_dimension_and_branch() {
    let raw = 0x830d_0020_u128;
    let source_chain = IntentNodeDraft::new(
        IntentNodeKind::Aggregate {
            aggregate: AggregateKind::OpenChain,
        },
        key("source_chain"),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 0),
        alias("source", IntentPortRole::Span, 0),
    );
    let offset = IntentNodeDraft::new(
        IntentNodeKind::Dimension {
            dimension: DimensionKind::ProfileOffset,
        },
        key("offset"),
    )
    .with_input(
        InputSlot::new(InputRole::Chain, 0),
        alias("source_chain", IntentPortRole::Chain, 0),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 0),
        alias("target", IntentPortRole::Span, 0),
    )
    .with_field(
        IntentFieldKey(key("mode")),
        IntentLiteral::Enum(key("driving")),
    )
    .with_field(
        IntentFieldKey(key("side")),
        IntentLiteral::Enum(key("left")),
    )
    .with_field(
        IntentFieldKey(key("source_traversal")),
        IntentLiteral::Enum(key("forward")),
    )
    .with_field(
        IntentFieldKey(key("target_traversal")),
        IntentLiteral::Enum(key("forward")),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Target, 0),
        LeafField::Value,
        coordinate(2.0),
    );
    let intent = intent_fixture(
        raw,
        vec![
            create("source", segment("source", [0.0, 0.0], [4.0, 0.0])),
            create("target", segment("target", [0.0, 2.0], [4.0, 2.0])),
            create("source_chain", source_chain),
            create("offset", offset),
        ],
    );

    let mut flat_document = SketchDocument::with_id(
        1.0,
        DocumentId(PersistentId::from_u128((raw + 0x1000) << 32)),
    )
    .unwrap();
    let source_start = flat_document.add_point("source start", [0.0, 0.0]).unwrap();
    let source_end = flat_document.add_point("source end", [4.0, 0.0]).unwrap();
    let source = flat_document
        .add_curve(
            "source",
            CurveDefinition::Line {
                start: source_start,
                end: source_end,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let target_start = flat_document.add_point("target start", [0.0, 2.0]).unwrap();
    let target_end = flat_document.add_point("target end", [4.0, 2.0]).unwrap();
    let target = flat_document
        .add_curve(
            "target",
            CurveDefinition::Line {
                start: target_start,
                end: target_end,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let operand = DocumentProfileOffsetOperand::OpenChain {
        side: DocumentLineSide::Left,
        chain: DocumentProfileOffsetChain {
            edges: vec![DocumentProfileOffsetEdgePair {
                source: DocumentDirectedProfileOffsetCurve {
                    curve: CurveSpan::line(source),
                    traversal: DocumentOffsetTraversal::Forward,
                },
                target: DocumentDirectedProfileOffsetCurve {
                    curve: CurveSpan::line(target),
                    traversal: DocumentOffsetTraversal::Forward,
                },
            }],
            junctions: Vec::new(),
            start_terminal: DocumentProfileOffsetTerminalPolicy::NormalTranslation,
            end_terminal: DocumentProfileOffsetTerminalPolicy::NormalTranslation,
        },
    };
    flat_document
        .add_profile_offset("offset", 2.0, operand)
        .unwrap();
    let flat = native_session(flat_document);
    assert_flat_parity("Profile Offset", &intent, &flat);

    let branch_signature = |document: &SketchDocument| {
        let geosolve_sketch::DocumentDimensionDefinition::ProfileOffset { operand, .. } =
            &document.dimensions()[0].definition
        else {
            panic!("expected Profile Offset dimension")
        };
        let DocumentProfileOffsetOperand::OpenChain { side, chain } = operand else {
            panic!("expected open-chain Profile Offset")
        };
        (
            *side,
            chain.edges[0].source.traversal,
            chain.edges[0].target.traversal,
            chain.start_terminal,
            chain.end_terminal,
            chain.edges.len(),
            chain.junctions.len(),
        )
    };
    assert_eq!(
        branch_signature(accepted(&intent.output.session).document()),
        branch_signature(accepted(&flat).document())
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "computed Fillet parity includes its explicit parent branches, generated arc and discarded source topology"
)]
fn computed_fillet_intent_matches_direct_flat_feature_authoring_and_cold_replay() {
    let raw = 0x830d_0030_u128;
    let mut intent = intent_fixture(
        raw,
        vec![
            create("start", sketch_point("start", [0.0, 0.0])),
            create("corner", sketch_point("corner", [4.0, 0.0])),
            create("end", sketch_point("end", [4.0, 4.0])),
            create(
                "first",
                segment_between("first", "start", "corner", [1.0, 0.0]),
            ),
            create(
                "second",
                segment_between("second", "corner", "end", [0.0, 1.0]),
            ),
        ],
    );
    let native_rank_before = accepted(&intent.output.session).diagnostics().rank;
    let native_mobility_before = accepted(&intent.output.session).diagnostics().mobility;
    let intent_candidate = fillet_candidate(&intent.output.session, 1.0);
    let intent_accepted = accepted(&intent.output.session);
    let translated = projectional_fillet_patch(
        intent.session.identity(),
        &intent.session,
        &intent.output.ownership,
        intent.output.session.accepted_prepared_input().unwrap(),
        intent_accepted.identity(),
        key("Fillet 1"),
        &intent_candidate,
    )
    .unwrap();
    let captured = RefCell::new(None);
    let plan = intent
        .session
        .plan_patch(translated.patch, |candidate| {
            let output = intent.materializer.materialize(candidate).unwrap();
            let evidence = output.evidence.clone();
            captured.replace(Some(output));
            IntentEvaluation::Accepted { evidence }
        })
        .unwrap();
    intent.session.commit_plan(plan).unwrap();
    intent.output = captured.into_inner().unwrap();
    assert_eq!(
        accepted(&intent.output.session).diagnostics().rank,
        native_rank_before,
        "computed features must not alter native solver rank"
    );
    assert_eq!(
        accepted(&intent.output.session).diagnostics().mobility,
        native_mobility_before,
        "computed features must not alter native solver mobility"
    );

    let mut flat_document = SketchDocument::with_id(
        1.0,
        DocumentId(PersistentId::from_u128((raw + 0x1000) << 32)),
    )
    .unwrap();
    let corner = flat_document.add_point("corner", [4.0, 0.0]).unwrap();
    let end = flat_document.add_point("end", [4.0, 4.0]).unwrap();
    flat_document
        .add_curve(
            "second",
            CurveDefinition::Line {
                start: corner,
                end,
                branch_direction: [0.0, 1.0],
            },
        )
        .unwrap();
    let start = flat_document.add_point("start", [0.0, 0.0]).unwrap();
    flat_document
        .add_curve(
            "first",
            CurveDefinition::Line {
                start,
                end: corner,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let flat = native_session(flat_document);
    let flat_candidate = fillet_candidate(&flat, 1.0);
    let mut flat_features = ComputedFeatureDocument::new(flat.design_document().id());
    flat_features
        .create_fillet_set(
            "Fillet 1",
            flat_candidate.radius(),
            flat_candidate.persistent_corners(),
        )
        .unwrap();
    let flat_computed = evaluate_features(&flat, &flat_features);

    assert_flat_parity("computed Fillet native sketch", &intent, &flat);
    let intent_document = accepted(&intent.output.session).document();
    let flat_document = accepted(&flat).document();
    let [intent_feature] = intent.output.features.features() else {
        panic!("intent must materialize one computed Fillet")
    };
    let [flat_feature] = flat_features.features() else {
        panic!("flat authoring must retain one computed Fillet")
    };
    let ComputedFeatureDefinition::FilletSet(intent_fillet) = &intent_feature.definition;
    let ComputedFeatureDefinition::FilletSet(flat_fillet) = &flat_feature.definition;
    assert_close(
        "computed Fillet radius",
        intent_fillet.radius,
        flat_fillet.radius,
    );
    assert_eq!(intent_fillet.corners.len(), flat_fillet.corners.len());
    for (index, (intent_corner, flat_corner)) in intent_fillet
        .corners
        .iter()
        .zip(&flat_fillet.corners)
        .enumerate()
    {
        assert_fillet_parent_parity(
            &format!("corner {index} first parent"),
            intent_document,
            intent_corner.first,
            flat_document,
            flat_corner.first,
        );
        assert_fillet_parent_parity(
            &format!("corner {index} second parent"),
            intent_document,
            intent_corner.second,
            flat_document,
            flat_corner.second,
        );
        assert_eq!(
            intent_corner.endpoint_order, flat_corner.endpoint_order,
            "corner {index} endpoint order"
        );
        assert_eq!(
            intent_corner.sweep, flat_corner.sweep,
            "corner {index} sweep"
        );
    }
    assert_computed_fillet_parity(
        intent_document,
        &intent.output.computed,
        flat_document,
        &flat_computed,
    );

    let canonical = intent.session.to_canonical_json().unwrap();
    let restored = IntentSession::from_json(&canonical).unwrap();
    let replayed = intent
        .materializer
        .materialize_accepted_authority(restored.accepted().unwrap())
        .unwrap();
    assert_eq!(replayed.features, intent.output.features);
    assert_computed_fillet_parity(
        accepted(&replayed.session).document(),
        &replayed.computed,
        intent_document,
        &intent.output.computed,
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the host-input differential keeps native ID prediction, exact payloads and both diagnostic families in one reviewable fixture"
)]
fn external_point_and_activation_parameter_match_exact_flat_host_inputs_and_cold_replay() {
    let raw = 0x830d_0040_u128;
    let document_raw = raw << 32;
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(raw)).unwrap();
    let materializer = ColdIntentMaterializer::with_default_policy(
        DocumentId(PersistentId::from_u128(document_raw)),
        1.0,
    )
    .unwrap();
    let declarations = || {
        let external = IntentNodeDraft::new(
            IntentNodeKind::External {
                external: ExternalIntentKind::Binding,
            },
            key("host point"),
        )
        .with_field(
            IntentFieldKey(key("feature_kind")),
            IntentLiteral::Enum(key("point")),
        );
        let snapshot = IntentNodeDraft::new(
            IntentNodeKind::External {
                external: ExternalIntentKind::SnapshotReference,
            },
            key("snapshot"),
        )
        .with_input(
            InputSlot::new(InputRole::External, 0),
            alias("host", IntentPortRole::External, 0),
        )
        .with_field(
            IntentFieldKey(key("snapshot_revision")),
            IntentLiteral::Natural(17),
        );
        let relation = IntentNodeDraft::new(
            IntentNodeKind::Constraint {
                constraint: ConstraintKind::ExternalPointCoincident,
            },
            key("external point"),
        )
        .with_input(
            InputSlot::new(InputRole::Point, 0),
            alias("point", IntentPortRole::Primary, 0),
        )
        .with_input(
            InputSlot::new(InputRole::External, 0),
            alias("snapshot", IntentPortRole::External, 0),
        );
        let parameter_binding = IntentNodeDraft::new(
            IntentNodeKind::Parameter {
                parameter: ParameterIntentKind::Binding,
            },
            key("point activation"),
        )
        .with_input(
            InputSlot::new(InputRole::Parameter, 0),
            alias("active", IntentPortRole::Parameter, 0),
        )
        .with_input(
            InputSlot::new(InputRole::Point, 0),
            alias("point", IntentPortRole::Primary, 0),
        );
        vec![
            create("host", external),
            create("snapshot", snapshot),
            create("point", sketch_point("point", [3.0, 4.0])),
            create("external_relation", relation),
            create("active", activation_parameter("active")),
            create("activation_binding", parameter_binding),
        ]
    };

    let predicted = RefCell::new(None);
    assert!(
        session
            .plan_patch(
                IntentPatch::new(
                    session.identity(),
                    IntentPatchPolicy::RequireAccepted,
                    declarations(),
                ),
                |candidate| {
                    let reservation_offset = |kind| {
                        candidate
                            .reservations()
                            .entries()
                            .values()
                            .position(|record| record.kind == kind)
                            .unwrap()
                    };
                    predicted.replace(Some((
                        DocumentExternalBindingId(PersistentId::from_u128(
                            document_raw
                                + u128::try_from(reservation_offset(
                                    IntentNativeReservationKind::ExternalBinding,
                                ))
                                .unwrap()
                                + 1,
                        )),
                        DocumentParameterId(PersistentId::from_u128(
                            document_raw
                                + u128::try_from(reservation_offset(
                                    IntentNativeReservationKind::Parameter,
                                ))
                                .unwrap()
                                + 1,
                        )),
                    )));
                    materializer.evaluate(candidate)
                },
            )
            .is_err(),
        "the planning pass without exact host payloads must reject"
    );
    let (intent_external, intent_parameter) = predicted.into_inner().unwrap();
    let intent_parameter_batch = ParameterBatch::new(
        23,
        vec![ParameterBatchEntry {
            parameter: intent_parameter,
            value: ParameterValue::Activation(true),
        }],
    )
    .unwrap();
    let intent_snapshots = ExternalSnapshotSet::new(
        17,
        vec![ExternalSnapshotEntry {
            binding: intent_external,
            source_revision: 17,
            source_digest: ExternalSnapshotDigest::from_bytes([0x83; 32]),
            feature: ExternalSnapshotFeatureV1::Point {
                position: [3.0, 4.0],
                scale: 1.0,
                resources: ExternalSnapshotResourcesV1 {
                    point_count: 1,
                    control_count: 0,
                    span_count: 0,
                },
            },
        }],
    )
    .unwrap();
    let inputs = IntentExternalInputs::new(
        ExternalInputRevision::from_raw(1),
        intent_parameter_batch
            .to_canonical_json()
            .unwrap()
            .into_bytes(),
        intent_snapshots.to_canonical_json().unwrap().into_bytes(),
    )
    .unwrap();
    let captured = RefCell::new(None);
    let plan = session
        .plan_patch(
            IntentPatch::new(
                session.identity(),
                IntentPatchPolicy::RequireAccepted,
                std::iter::once(IntentPatchOperation::ReplaceExternalInputs { inputs })
                    .chain(declarations())
                    .collect(),
            ),
            |candidate| {
                let output = materializer.materialize(candidate).unwrap();
                let evidence = output.evidence.clone();
                captured.replace(Some(output));
                IntentEvaluation::Accepted { evidence }
            },
        )
        .unwrap();
    session.commit_plan(plan).unwrap();
    let intent = IntentFixture {
        session,
        materializer,
        output: captured.into_inner().unwrap(),
    };

    let mut flat_document = SketchDocument::with_id(
        1.0,
        DocumentId(PersistentId::from_u128((raw + 0x1000) << 32)),
    )
    .unwrap();
    let flat_external = flat_document
        .add_external_binding("host point", ExternalFeatureKindV1::Point, None)
        .unwrap();
    let flat_point = flat_document.add_point("point", [3.0, 4.0]).unwrap();
    flat_document
        .add_constraint(
            "external point",
            DocumentConstraintDefinition::ExternalPointCoincident {
                point: flat_point,
                external: DocumentExternalPointRef {
                    binding: flat_external,
                },
            },
        )
        .unwrap();
    let flat_parameter = flat_document
        .add_parameter("active", DocumentParameterKind::Activation)
        .unwrap();
    flat_document
        .add_parameter_binding(
            flat_parameter,
            DocumentParameterTarget::Activation(DocumentElementId::Point(flat_point)),
        )
        .unwrap();
    let flat_parameter_batch = ParameterBatch::new(
        23,
        vec![ParameterBatchEntry {
            parameter: flat_parameter,
            value: ParameterValue::Activation(true),
        }],
    )
    .unwrap();
    let flat_snapshots = ExternalSnapshotSet::new(
        17,
        vec![ExternalSnapshotEntry {
            binding: flat_external,
            source_revision: 17,
            source_digest: ExternalSnapshotDigest::from_bytes([0x83; 32]),
            feature: ExternalSnapshotFeatureV1::Point {
                position: [3.0, 4.0],
                scale: 1.0,
                resources: ExternalSnapshotResourcesV1 {
                    point_count: 1,
                    control_count: 0,
                    span_count: 0,
                },
            },
        }],
    )
    .unwrap();
    let flat = RetainedSketchDocumentSession::new_with_inputs(
        flat_document,
        flat_parameter_batch.clone(),
        flat_snapshots.clone(),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();

    assert_flat_parity("external point plus activation parameter", &intent, &flat);
    let intent_accepted = accepted(&intent.output.session);
    let flat_accepted = accepted(&flat);
    for (label, state) in [("intent", intent_accepted), ("flat", flat_accepted)] {
        assert_eq!(state.document().external_bindings().len(), 1, "{label}");
        assert_eq!(state.document().parameters().len(), 1, "{label}");
        assert_eq!(state.document().parameter_bindings().len(), 1, "{label}");
        assert_close(
            &format!("{label} external point x"),
            state.document().points()[0].position[0],
            3.0,
        );
        assert_close(
            &format!("{label} external point y"),
            state.document().points()[0].position[1],
            4.0,
        );
        let diagnostics = state.diagnostics();
        let [parameter] = diagnostics.parameters.as_slice() else {
            panic!("{label}: expected one parameter diagnostic")
        };
        assert_eq!(parameter.kind, DocumentParameterKind::Activation, "{label}");
        assert_eq!(parameter.state, SketchParameterState::Applied, "{label}");
        assert!(matches!(
            parameter.targets.as_slice(),
            [DocumentParameterTarget::Activation(
                DocumentElementId::Point(_)
            )]
        ));
        let [external] = diagnostics.external_references.as_slice() else {
            panic!("{label}: expected one external diagnostic")
        };
        assert_eq!(
            external.expected_kind,
            ExternalFeatureKindV1::Point,
            "{label}"
        );
        assert_eq!(
            external.state,
            SketchExternalReferenceState::Available,
            "{label}"
        );
    }
    assert_eq!(
        intent.output.session.parameter_batch(),
        &intent_parameter_batch
    );
    assert_eq!(
        intent.output.session.external_snapshot_set(),
        &intent_snapshots
    );
    assert_eq!(flat.parameter_batch(), &flat_parameter_batch);
    assert_eq!(flat.external_snapshot_set(), &flat_snapshots);
    assert_eq!(
        intent.output.session.parameter_batch().revision(),
        flat.parameter_batch().revision()
    );
    assert_eq!(
        intent.output.session.external_snapshot_set().revision(),
        flat.external_snapshot_set().revision()
    );
    assert_eq!(
        intent.output.session.external_snapshot_set().entries()[0].source_digest,
        flat.external_snapshot_set().entries()[0].source_digest
    );

    let canonical = intent.session.to_canonical_json().unwrap();
    let restored = IntentSession::from_json(&canonical).unwrap();
    assert_eq!(
        restored.external_inputs().identity(),
        intent.session.external_inputs().identity()
    );
    let replayed = intent
        .materializer
        .materialize_accepted_authority(restored.accepted().unwrap())
        .unwrap();
    assert_eq!(replayed.session.parameter_batch(), &intent_parameter_batch);
    assert_eq!(replayed.session.external_snapshot_set(), &intent_snapshots);
    assert_eq!(replayed.evidence, intent.output.evidence);
    assert_eq!(replayed.validation, intent.output.validation);
}
