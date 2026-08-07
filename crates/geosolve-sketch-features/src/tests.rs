// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch::{
    CancellationToken, CurveDefinition, CurveSpan, DocumentArcSweep, DocumentCurveNormalSide,
    DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint, DocumentObjectId,
    DocumentSolveRequest, OperationControl, OperationLimits, OperationOutcome,
    RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDocument, SolverConfig,
    cancellation_pair,
};

use crate::{
    ComputedEdgeGeometry, ComputedEvaluationAllocator, ComputedEvaluationAllocatorHighWater,
    ComputedEvaluationRevision, ComputedFeatureDefinition, ComputedFeatureDocument,
    ComputedFeatureDocumentError, ComputedFeatureDocumentId, ComputedFeatureEvaluationPolicy,
    ComputedFeatureEvaluationSnapshot, ComputedFeatureEvaluationState, ComputedFeatureFailure,
    ComputedFeatureLifecycleHighWater, ComputedFeatureRevision, ComputedFilletAuthoringOptions,
    ComputedFilletCornerAuthoringRequest, ComputedFilletCurvePick, ComputedFilletParent,
    NativeCurveSpanSource, NewComputedFilletCorner,
};

struct PolylineFixture {
    document: SketchDocument,
    curve: geosolve_sketch::CurveId,
    spans: [CurveSpan; 3],
}

fn polyline_fixture() -> PolylineFixture {
    let mut document = SketchDocument::with_id(
        10.0,
        geosolve_sketch::DocumentId(geosolve_sketch::PersistentId::from_u128(0x1000)),
    )
    .unwrap();
    let points = [
        document.add_point("p0", [0.0, 0.0]).unwrap(),
        document.add_point("p1", [4.0, 0.0]).unwrap(),
        document.add_point("p2", [4.0, 4.0]).unwrap(),
        document.add_point("p3", [8.0, 4.0]).unwrap(),
    ];
    let curve = document
        .add_curve(
            "three span polyline",
            CurveDefinition::Polyline {
                points: points.to_vec(),
                closed: false,
                branch_directions: vec![[1.0, 0.0], [0.0, 1.0], [1.0, 0.0]],
            },
        )
        .unwrap();
    PolylineFixture {
        document,
        curve,
        spans: [0, 1, 2].map(|segment| CurveSpan { curve, segment }),
    }
}

fn retained(document: SketchDocument) -> RetainedSketchDocumentSession {
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    assert!(session.accepted_state_for_current_input().is_some());
    session
}

fn source(span: CurveSpan) -> NativeCurveSpanSource {
    NativeCurveSpanSource { span }
}

fn parent(
    span: CurveSpan,
    picked_parameter: f64,
    normal_side: DocumentCurveNormalSide,
    retained_endpoint: DocumentFilletTrimEndpoint,
) -> ComputedFilletParent {
    ComputedFilletParent {
        source: source(span),
        picked_parameter,
        winding: 0,
        neighborhood: geosolve_sketch::ContactNeighborhood::Interior,
        normal_side,
        retained_endpoint,
        periodic_anchor: None,
    }
}

fn first_corner(spans: [CurveSpan; 3]) -> NewComputedFilletCorner {
    NewComputedFilletCorner {
        first: parent(
            spans[0],
            0.875,
            DocumentCurveNormalSide::Left,
            DocumentFilletTrimEndpoint::End,
        ),
        second: parent(
            spans[1],
            0.125,
            DocumentCurveNormalSide::Left,
            DocumentFilletTrimEndpoint::Start,
        ),
        endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
        sweep: DocumentArcSweep::CounterClockwise,
    }
}

fn second_corner(spans: [CurveSpan; 3]) -> NewComputedFilletCorner {
    NewComputedFilletCorner {
        first: parent(
            spans[1],
            0.875,
            DocumentCurveNormalSide::Right,
            DocumentFilletTrimEndpoint::End,
        ),
        second: parent(
            spans[2],
            0.125,
            DocumentCurveNormalSide::Right,
            DocumentFilletTrimEndpoint::Start,
        ),
        endpoint_order: DocumentFilletEndpointOrder::SecondThenFirst,
        sweep: DocumentArcSweep::CounterClockwise,
    }
}

fn add_independent_corner(document: &mut SketchDocument) -> [CurveSpan; 2] {
    let points = [
        document.add_point("q0", [10.0, 0.0]).unwrap(),
        document.add_point("q1", [14.0, 0.0]).unwrap(),
        document.add_point("q2", [14.0, 4.0]).unwrap(),
    ];
    let curve = document
        .add_curve(
            "independent polyline",
            CurveDefinition::Polyline {
                points: points.to_vec(),
                closed: false,
                branch_directions: vec![[1.0, 0.0], [0.0, 1.0]],
            },
        )
        .unwrap();
    [0, 1].map(|segment| CurveSpan { curve, segment })
}

fn independent_corner(spans: [CurveSpan; 2]) -> NewComputedFilletCorner {
    NewComputedFilletCorner {
        first: parent(
            spans[0],
            0.875,
            DocumentCurveNormalSide::Left,
            DocumentFilletTrimEndpoint::End,
        ),
        second: parent(
            spans[1],
            0.125,
            DocumentCurveNormalSide::Left,
            DocumentFilletTrimEndpoint::Start,
        ),
        endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
        sweep: DocumentArcSweep::CounterClockwise,
    }
}

fn complete<T: std::fmt::Debug>(outcome: OperationOutcome<T>) -> T {
    match outcome {
        OperationOutcome::Completed { value, .. } => value,
        other => panic!("expected completed operation, got {other:?}"),
    }
}

fn evaluate(
    session: &RetainedSketchDocumentSession,
    features: &ComputedFeatureDocument,
    allocator: &mut ComputedEvaluationAllocator,
) -> crate::ComputedFeatureSnapshot {
    let snapshot = ComputedFeatureEvaluationSnapshot::capture(
        session,
        features,
        ComputedFeatureEvaluationPolicy::default(),
    )
    .unwrap();
    complete(
        snapshot
            .prepare(allocator)
            .unwrap()
            .execute(OperationControl::unlimited())
            .unwrap(),
    )
}

fn arc_centers(snapshot: &crate::ComputedFeatureSnapshot) -> Vec<[u64; 2]> {
    let mut centers = snapshot
        .edges()
        .iter()
        .filter_map(|edge| match &edge.geometry {
            ComputedEdgeGeometry::CircularArc(arc) => {
                Some([arc.center[0].to_bits(), arc.center[1].to_bits()])
            }
            ComputedEdgeGeometry::NativeSourceFragment { .. } => None,
        })
        .collect::<Vec<_>>();
    centers.sort_unstable();
    centers
}

#[test]
fn strict_json_digest_duplicate_pairs_and_lifecycle_high_water() {
    let fixture = polyline_fixture();
    let mut features = ComputedFeatureDocument::with_id(
        fixture.document.id(),
        ComputedFeatureDocumentId::from_raw(0xabc),
    );
    let checkpoint = features.clone();
    let first = features
        .create_fillet_set("corners", 0.5, vec![first_corner(fixture.spans)])
        .unwrap();
    let first_corner_id = match &features.feature(first).unwrap().definition {
        ComputedFeatureDefinition::FilletSet(fillet) => fillet.corners[0].id,
    };
    assert!(matches!(
        features.add_fillet_corners(first, vec![first_corner(fixture.spans)]),
        Err(ComputedFeatureDocumentError::InvalidField {
            field: "corner parents",
            ..
        })
    ));

    let json = features.to_json().unwrap();
    assert_eq!(ComputedFeatureDocument::from_json(&json).unwrap(), features);
    let mut unknown: serde_json::Value = serde_json::from_str(&json).unwrap();
    unknown["misleading_old_field"] = serde_json::json!(true);
    assert!(matches!(
        ComputedFeatureDocument::from_json(&serde_json::to_string(&unknown).unwrap()),
        Err(ComputedFeatureDocumentError::Json(_))
    ));
    let tampered = json.replace("\"radius\":0.5", "\"radius\":0.6");
    assert!(matches!(
        ComputedFeatureDocument::from_json(&tampered),
        Err(ComputedFeatureDocumentError::DigestMismatch)
    ));

    let retained_high_water = features.lifecycle_high_water();
    let mut restored = checkpoint;
    restored.rebase_after_restore(retained_high_water).unwrap();
    assert!(restored.revision() > retained_high_water.revision);
    let replacement = restored
        .create_fillet_set("replacement", 0.5, vec![first_corner(fixture.spans)])
        .unwrap();
    let replacement_corner = match &restored.feature(replacement).unwrap().definition {
        ComputedFeatureDefinition::FilletSet(fillet) => fillet.corners[0].id,
    };
    assert!(replacement > first);
    assert!(replacement_corner > first_corner_id);
}

#[test]
fn reversed_parent_order_is_canonical_without_changing_arc_endpoint_semantics() {
    let fixture = polyline_fixture();
    let original = first_corner(fixture.spans);
    let reversed = NewComputedFilletCorner {
        first: original.second,
        second: original.first,
        endpoint_order: DocumentFilletEndpointOrder::SecondThenFirst,
        sweep: original.sweep,
    };
    assert_eq!(reversed.canonicalized(), original);
    let mut features = ComputedFeatureDocument::new(fixture.document.id());
    let id = features
        .create_fillet_set("reverse", 0.5, vec![reversed])
        .unwrap();
    let ComputedFeatureDefinition::FilletSet(fillet) = &features.feature(id).unwrap().definition;
    assert_eq!(fillet.corners[0].without_id(), original);
}

#[test]
fn adjacent_batch_composes_both_middle_endpoints_and_variable_output_count() {
    let fixture = polyline_fixture();
    let session = retained(fixture.document.clone());
    let mut features = ComputedFeatureDocument::new(fixture.document.id());
    let feature = features
        .create_fillet_set(
            "both corners",
            0.5,
            vec![first_corner(fixture.spans), second_corner(fixture.spans)],
        )
        .unwrap();
    let mut allocator = ComputedEvaluationAllocator::default();
    let snapshot = evaluate(&session, &features, &mut allocator);
    assert_eq!(snapshot.edges().len(), 5);
    assert_eq!(arc_centers(&snapshot).len(), 2);
    let middle = snapshot
        .source_fragment_edges(source(fixture.spans[1]))
        .next()
        .unwrap();
    let crate::ComputedEdgeProvenance::SourceFragment {
        interval,
        start_claim,
        end_claim,
        ..
    } = &middle.provenance
    else {
        panic!("expected source fragment");
    };
    assert!(start_claim.is_some() && end_claim.is_some());
    assert!((interval.start - 0.125).abs() < 1.0e-8);
    assert!((interval.end - 0.875).abs() < 1.0e-8);
    let state = &snapshot
        .feature_evaluations()
        .iter()
        .find(|value| value.feature == feature)
        .unwrap()
        .state;
    assert!(matches!(
        state,
        ComputedFeatureEvaluationState::Current { corner_edges } if corner_edges.len() == 2
    ));
}

#[test]
fn sequential_sets_match_batch_geometry_and_suppression_is_local() {
    let fixture = polyline_fixture();
    let session = retained(fixture.document.clone());
    let mut batch = ComputedFeatureDocument::new(fixture.document.id());
    batch
        .create_fillet_set(
            "batch",
            0.5,
            vec![first_corner(fixture.spans), second_corner(fixture.spans)],
        )
        .unwrap();
    let mut sequential = ComputedFeatureDocument::new(fixture.document.id());
    let first = sequential
        .create_fillet_set("first", 0.5, vec![first_corner(fixture.spans)])
        .unwrap();
    sequential
        .create_fillet_set("second", 0.5, vec![second_corner(fixture.spans)])
        .unwrap();
    let mut allocator = ComputedEvaluationAllocator::default();
    let batch_snapshot = evaluate(&session, &batch, &mut allocator);
    let sequential_snapshot = evaluate(&session, &sequential, &mut allocator);
    assert_eq!(
        arc_centers(&batch_snapshot),
        arc_centers(&sequential_snapshot)
    );
    assert_ne!(
        batch_snapshot.evaluation_revision(),
        sequential_snapshot.evaluation_revision()
    );
    assert!(
        batch_snapshot
            .edge(sequential_snapshot.edges()[0].id)
            .is_none()
    );

    sequential.set_suppressed(first, true).unwrap();
    let suppressed = evaluate(&session, &sequential, &mut allocator);
    assert_eq!(arc_centers(&suppressed).len(), 1);
    assert!(matches!(
        suppressed
            .feature_evaluations()
            .iter()
            .find(|value| value.feature == first)
            .unwrap()
            .state,
        ComputedFeatureEvaluationState::Suppressed
    ));
}

#[test]
fn crossed_claims_fail_whole_set_atomically_and_radius_edit_recovers() {
    let fixture = polyline_fixture();
    let session = retained(fixture.document.clone());
    let mut features = ComputedFeatureDocument::new(fixture.document.id());
    let feature = features
        .create_fillet_set(
            "conflict",
            2.5,
            vec![first_corner(fixture.spans), second_corner(fixture.spans)],
        )
        .unwrap();
    let mut allocator = ComputedEvaluationAllocator::default();
    let failed = evaluate(&session, &features, &mut allocator);
    assert!(arc_centers(&failed).is_empty());
    assert!(matches!(
        &failed
            .feature_evaluations()
            .iter()
            .find(|value| value.feature == feature)
            .unwrap()
            .state,
        ComputedFeatureEvaluationState::Failed {
            failure: ComputedFeatureFailure::ConsumedSourceInterval { .. }
        }
    ));
    features.set_fillet_radius(feature, 0.5).unwrap();
    let recovered = evaluate(&session, &features, &mut allocator);
    assert_eq!(arc_centers(&recovered).len(), 2);
}

#[test]
fn missing_source_is_repairable_and_does_not_block_unrelated_feature() {
    let mut fixture = polyline_fixture();
    let independent = add_independent_corner(&mut fixture.document);
    let original = retained(fixture.document.clone());
    let mut features = ComputedFeatureDocument::new(fixture.document.id());
    let missing = features
        .create_fillet_set("will be missing", 0.5, vec![first_corner(fixture.spans)])
        .unwrap();
    let other = features
        .create_fillet_set("survives", 0.5, vec![independent_corner(independent)])
        .unwrap();
    let mut deleted = fixture.document.clone();
    deleted
        .remove_with_owned_state(DocumentObjectId::Curve(fixture.curve))
        .unwrap();
    let deleted = retained(deleted);
    let mut allocator = ComputedEvaluationAllocator::default();
    let failed = evaluate(&deleted, &features, &mut allocator);
    assert!(matches!(
        failed
            .feature_evaluations()
            .iter()
            .find(|value| value.feature == missing)
            .unwrap()
            .state,
        ComputedFeatureEvaluationState::Failed {
            failure: ComputedFeatureFailure::MissingSource { .. }
        }
    ));
    assert!(matches!(
        failed
            .feature_evaluations()
            .iter()
            .find(|value| value.feature == other)
            .unwrap()
            .state,
        ComputedFeatureEvaluationState::Current { .. }
    ));
    assert_eq!(arc_centers(&failed).len(), 1);
    let recovered = evaluate(&original, &features, &mut allocator);
    assert!(recovered.feature_evaluations().iter().any(|value| {
        value.feature == missing
            && matches!(value.state, ComputedFeatureEvaluationState::Current { .. })
    }));
    assert!(recovered.feature_evaluations().iter().any(|value| {
        value.feature == other
            && matches!(value.state, ComputedFeatureEvaluationState::Current { .. })
    }));
}

#[test]
fn evaluation_allocator_persists_nonreuse_and_control_stops_without_output() {
    let fixture = polyline_fixture();
    let session = retained(fixture.document.clone());
    let mut features = ComputedFeatureDocument::new(fixture.document.id());
    features
        .create_fillet_set("one", 0.5, vec![first_corner(fixture.spans)])
        .unwrap();
    let mut allocator = ComputedEvaluationAllocator::default();
    let first = evaluate(&session, &features, &mut allocator);
    let high_water = allocator.high_water();
    let encoded = serde_json::to_string(&high_water).unwrap();
    let restored: ComputedEvaluationAllocatorHighWater = serde_json::from_str(&encoded).unwrap();
    let mut restored_allocator = ComputedEvaluationAllocator::from_high_water(restored);
    let second = evaluate(&session, &features, &mut restored_allocator);
    assert!(second.evaluation_revision() > first.evaluation_revision());

    let snapshot = ComputedFeatureEvaluationSnapshot::capture(
        &session,
        &features,
        ComputedFeatureEvaluationPolicy::default(),
    )
    .unwrap();
    let (handle, token) = cancellation_pair();
    handle.cancel();
    assert!(matches!(
        snapshot
            .clone()
            .prepare(&mut restored_allocator)
            .unwrap()
            .execute(OperationControl::new(token, OperationLimits::unlimited()))
            .unwrap(),
        OperationOutcome::Cancelled { .. }
    ));
    let mut limits = OperationLimits::unlimited();
    limits.document_validation_items = 0;
    assert!(matches!(
        snapshot
            .prepare(&mut restored_allocator)
            .unwrap()
            .execute(OperationControl::new(CancellationToken::default(), limits))
            .unwrap(),
        OperationOutcome::WorkExhausted { .. }
    ));
}

#[test]
fn authoring_owns_root_selection_and_rejects_fabricated_pick_positions() {
    let fixture = polyline_fixture();
    let session = retained(fixture.document.clone());
    let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
    let first_jet = fixture
        .document
        .evaluate_curve_jet(fixture.spans[0], 0.75)
        .unwrap();
    let second_jet = fixture
        .document
        .evaluate_curve_jet(fixture.spans[1], 0.25)
        .unwrap();
    let request = ComputedFilletCornerAuthoringRequest {
        first: ComputedFilletCurvePick {
            source: source(fixture.spans[0]),
            parameter: 0.75,
            model_position: [first_jet.position.x, first_jet.position.y],
            retained_endpoint_hint: Some(DocumentFilletTrimEndpoint::End),
        },
        second: ComputedFilletCurvePick {
            source: source(fixture.spans[1]),
            parameter: 0.25,
            model_position: [second_jet.position.x, second_jet.position.y],
            retained_endpoint_hint: Some(DocumentFilletTrimEndpoint::Start),
        },
        options: ComputedFilletAuthoringOptions::default(),
    };
    let resolved = complete(
        authoring
            .resolve_fillet_corner(
                request,
                0.5,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::unlimited(),
            )
            .unwrap(),
    );
    assert!((resolved.arc.radius - 0.5).abs() < 1.0e-12);
    assert_eq!(resolved.corner.first.source, source(fixture.spans[0]));

    let mut fabricated = request;
    fabricated.first.model_position[0] += 1.0;
    assert!(matches!(
        authoring.resolve_fillet_corner(
            fabricated,
            0.5,
            ComputedFeatureEvaluationPolicy::default(),
            OperationControl::unlimited(),
        ),
        Err(crate::ComputedFeatureAuthoringError::StalePick)
    ));
}

#[test]
fn affine_non_affine_authoring_certifies_and_persists_periodic_branch_state() {
    let mut document = SketchDocument::with_id(
        10.0,
        geosolve_sketch::DocumentId(geosolve_sketch::PersistentId::from_u128(0x2000)),
    )
    .unwrap();
    let line_start = document.add_point("line start", [-5.0, 0.0]).unwrap();
    let line_end = document.add_point("line end", [5.0, 0.0]).unwrap();
    let line = CurveSpan::line(
        document
            .add_curve(
                "line",
                CurveDefinition::Line {
                    start: line_start,
                    end: line_end,
                    branch_direction: [1.0, 0.0],
                },
            )
            .unwrap(),
    );
    let center = document.add_point("circle center", [0.0, 2.0]).unwrap();
    let circle_radius = document
        .add_scalar(
            "circle radius",
            1.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let circle = CurveSpan::line(
        document
            .add_curve(
                "circle",
                CurveDefinition::Circle {
                    center,
                    radius: circle_radius,
                },
            )
            .unwrap(),
    );
    let session = retained(document.clone());
    let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
    let line_parameter = 0.4;
    let circle_parameter = 4.5;
    let line_jet = document.evaluate_curve_jet(line, line_parameter).unwrap();
    let circle_jet = document
        .evaluate_curve_jet(circle, circle_parameter)
        .unwrap();
    let resolved = complete(
        authoring
            .resolve_fillet_corner(
                ComputedFilletCornerAuthoringRequest {
                    first: ComputedFilletCurvePick {
                        source: source(line),
                        parameter: line_parameter,
                        model_position: [line_jet.position.x, line_jet.position.y],
                        retained_endpoint_hint: Some(DocumentFilletTrimEndpoint::End),
                    },
                    second: ComputedFilletCurvePick {
                        source: source(circle),
                        parameter: circle_parameter,
                        model_position: [circle_jet.position.x, circle_jet.position.y],
                        retained_endpoint_hint: Some(DocumentFilletTrimEndpoint::End),
                    },
                    options: ComputedFilletAuthoringOptions::default(),
                },
                0.5,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::unlimited(),
            )
            .unwrap(),
    );
    assert!(resolved.corner.second.periodic_anchor.is_some());
    assert!(matches!(
        resolved.corner.second.neighborhood,
        geosolve_sketch::ContactNeighborhood::Local { .. }
    ));
    let mut features = ComputedFeatureDocument::new(document.id());
    let feature = features
        .create_fillet_set("line circle", 0.5, vec![resolved.corner])
        .unwrap();
    let mut allocator = ComputedEvaluationAllocator::default();
    let snapshot = evaluate(&session, &features, &mut allocator);
    assert!(matches!(
        snapshot
            .feature_evaluations()
            .iter()
            .find(|value| value.feature == feature)
            .unwrap()
            .state,
        ComputedFeatureEvaluationState::Current { .. }
    ));
    assert_eq!(arc_centers(&snapshot).len(), 1);
}

#[test]
fn explicit_lifecycle_high_water_rebase_is_strictly_monotonic() {
    let fixture = polyline_fixture();
    let mut document = ComputedFeatureDocument::new(fixture.document.id());
    document
        .rebase_after_restore(ComputedFeatureLifecycleHighWater {
            revision: ComputedFeatureRevision::from_raw(50),
            allocator: document.allocator_high_water(),
        })
        .unwrap();
    assert_eq!(document.revision().raw(), 51);
    let mut allocator = ComputedEvaluationAllocator::default();
    allocator.retain_high_water(ComputedEvaluationAllocatorHighWater {
        next_revision: ComputedEvaluationRevision::from_raw(42),
    });
    assert_eq!(allocator.high_water().next_revision.raw(), 42);
}
