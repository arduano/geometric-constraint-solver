// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch::{
    CancellationToken, CurveDefinition, CurveSpan, DocumentArcSweep, DocumentCurveNormalSide,
    DocumentCurveTrimView, DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint,
    DocumentObjectId, DocumentSolveRequest, DocumentTrimBoundary, DocumentTrimParameter,
    OperationControl, OperationLimits, OperationOutcome, RetainedSketchDocumentSession,
    ScalarDomain, ScalarUnit, SketchDocument, SolverConfig, cancellation_pair,
};

use crate::{
    ComputedEdgeGeometry, ComputedEvaluationAllocator, ComputedEvaluationAllocatorHighWater,
    ComputedEvaluationRevision, ComputedFeatureAllocatorHighWater, ComputedFeatureAuthoringError,
    ComputedFeatureDefinition, ComputedFeatureDocument, ComputedFeatureDocumentError,
    ComputedFeatureDocumentId, ComputedFeatureEvaluationPolicy, ComputedFeatureEvaluationSnapshot,
    ComputedFeatureEvaluationState, ComputedFeatureFailure, ComputedFeatureLifecycleHighWater,
    ComputedFeatureRevision, ComputedFeatureSnapshotError, ComputedFilletAuthoringOptions,
    ComputedFilletCornerAuthoringRequest, ComputedFilletCurvePick, ComputedFilletParent,
    NativeCurveSpanSource, NewComputedFilletCorner,
};

struct PolylineFixture {
    document: SketchDocument,
    curve: geosolve_sketch::CurveId,
    spans: [CurveSpan; 3],
}

fn polyline_fixture() -> PolylineFixture {
    scaled_polyline_fixture(1.0, 0x1000)
}

fn scaled_polyline_fixture(scale: f64, document_id: u128) -> PolylineFixture {
    let mut document = SketchDocument::with_id(
        10.0 * scale,
        geosolve_sketch::DocumentId(geosolve_sketch::PersistentId::from_u128(document_id)),
    )
    .unwrap();
    let points = [
        document.add_point("p0", [0.0, 0.0]).unwrap(),
        document.add_point("p1", [4.0 * scale, 0.0]).unwrap(),
        document
            .add_point("p2", [4.0 * scale, 4.0 * scale])
            .unwrap(),
        document
            .add_point("p3", [8.0 * scale, 4.0 * scale])
            .unwrap(),
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

fn first_corner_authoring_request(
    document: &SketchDocument,
    spans: [CurveSpan; 3],
) -> ComputedFilletCornerAuthoringRequest {
    let first_parameter = 0.75;
    let second_parameter = 0.25;
    let first_jet = document
        .evaluate_curve_jet(spans[0], first_parameter)
        .unwrap();
    let second_jet = document
        .evaluate_curve_jet(spans[1], second_parameter)
        .unwrap();
    ComputedFilletCornerAuthoringRequest {
        first: ComputedFilletCurvePick {
            source: source(spans[0]),
            parameter: first_parameter,
            model_position: [first_jet.position.x, first_jet.position.y],
            retained_endpoint_hint: Some(DocumentFilletTrimEndpoint::End),
        },
        second: ComputedFilletCurvePick {
            source: source(spans[1]),
            parameter: second_parameter,
            model_position: [second_jet.position.x, second_jet.position.y],
            retained_endpoint_hint: Some(DocumentFilletTrimEndpoint::Start),
        },
        options: ComputedFilletAuthoringOptions::default(),
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

struct LineCircleFixture {
    document: SketchDocument,
    line: CurveSpan,
    circle: CurveSpan,
    request: ComputedFilletCornerAuthoringRequest,
}

fn line_circle_fixture() -> LineCircleFixture {
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
    let line_parameter = 0.4;
    let circle_parameter = 4.5;
    let line_jet = document.evaluate_curve_jet(line, line_parameter).unwrap();
    let circle_jet = document
        .evaluate_curve_jet(circle, circle_parameter)
        .unwrap();
    let request = ComputedFilletCornerAuthoringRequest {
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
    };
    LineCircleFixture {
        document,
        line,
        circle,
        request,
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

fn evaluate_single_corner_failure(
    document: &SketchDocument,
    radius: f64,
    corner: NewComputedFilletCorner,
) -> ComputedFeatureFailure {
    let session = retained(document.clone());
    let mut features = ComputedFeatureDocument::new(document.id());
    let feature = features
        .create_fillet_set("failure", radius, vec![corner])
        .unwrap();
    let mut allocator = ComputedEvaluationAllocator::default();
    let snapshot = evaluate(&session, &features, &mut allocator);
    match snapshot
        .feature_evaluations()
        .iter()
        .find(|value| value.feature == feature)
        .unwrap()
        .state
        .clone()
    {
        ComputedFeatureEvaluationState::Failed { failure } => failure,
        other => panic!("expected failed feature, got {other:?}"),
    }
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

type SourceGeometrySignature = (NativeCurveSpanSource, u64, u64);
type ComputedGeometrySignature = (Vec<SourceGeometrySignature>, Vec<Vec<u64>>);

fn geometry_signature(snapshot: &crate::ComputedFeatureSnapshot) -> ComputedGeometrySignature {
    let mut sources = Vec::new();
    let mut arcs = Vec::new();
    for edge in snapshot.edges() {
        match &edge.geometry {
            ComputedEdgeGeometry::NativeSourceFragment { source, interval } => {
                sources.push((*source, interval.start.to_bits(), interval.end.to_bits()));
            }
            ComputedEdgeGeometry::CircularArc(arc) => {
                let mut signature = vec![
                    arc.center[0].to_bits(),
                    arc.center[1].to_bits(),
                    arc.radius.to_bits(),
                    arc.start_angle.to_bits(),
                    arc.end_angle.to_bits(),
                    match arc.sweep {
                        DocumentArcSweep::CounterClockwise => 0,
                        DocumentArcSweep::Clockwise => 1,
                    },
                ];
                for contact in arc.contacts {
                    signature.extend([
                        contact.parameter.to_bits(),
                        u64::from_ne_bytes(i64::from(contact.winding).to_ne_bytes()),
                        contact.total_parameter.to_bits(),
                        contact.position[0].to_bits(),
                        contact.position[1].to_bits(),
                    ]);
                }
                arcs.push(signature);
            }
        }
    }
    sources.sort_unstable();
    arcs.sort_unstable();
    (sources, arcs)
}

fn saturated_revision(mut document: ComputedFeatureDocument) -> ComputedFeatureDocument {
    document.set_revision_for_test(ComputedFeatureRevision::from_raw(u64::MAX));
    document
}

fn assert_revision_exhaustion_rolls_back<T>(
    document: ComputedFeatureDocument,
    mutate: impl FnOnce(&mut ComputedFeatureDocument) -> Result<T, ComputedFeatureDocumentError>,
) {
    let mut document = saturated_revision(document);
    let before = document.clone();
    assert!(matches!(
        mutate(&mut document),
        Err(ComputedFeatureDocumentError::RevisionExhausted)
    ));
    assert_eq!(document, before);
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
fn every_revision_exhaustion_path_is_transactional() {
    let fixture = polyline_fixture();
    let mut base = ComputedFeatureDocument::new(fixture.document.id());
    let feature = base
        .create_fillet_set("first", 0.5, vec![first_corner(fixture.spans)])
        .unwrap();
    let corner = match &base.feature(feature).unwrap().definition {
        ComputedFeatureDefinition::FilletSet(fillet) => fillet.corners[0].id,
    };

    assert_revision_exhaustion_rolls_back(base.clone(), |document| {
        document.create_fillet_set("second", 0.5, vec![second_corner(fixture.spans)])
    });
    assert_revision_exhaustion_rolls_back(base.clone(), |document| {
        document.add_fillet_corners(feature, vec![second_corner(fixture.spans)])
    });
    assert_revision_exhaustion_rolls_back(base.clone(), |document| {
        document.set_fillet_radius(feature, 0.75)
    });
    let mut replacement = first_corner(fixture.spans);
    replacement.sweep = DocumentArcSweep::Clockwise;
    assert_revision_exhaustion_rolls_back(base.clone(), |document| {
        document.set_fillet_corner(feature, corner, replacement)
    });
    assert_revision_exhaustion_rolls_back(base.clone(), |document| {
        document.set_suppressed(feature, true)
    });
    assert_revision_exhaustion_rolls_back(base.clone(), |document| {
        document.set_label(feature, "renamed")
    });
    assert_revision_exhaustion_rolls_back(base.clone(), |document| {
        document.remove_corner(feature, corner)
    });
    assert_revision_exhaustion_rolls_back(base.clone(), |document| {
        document.remove_feature(feature)
    });
    let allocator = base.allocator_high_water();
    assert_revision_exhaustion_rolls_back(base.clone(), |document| {
        document.retain_allocator_high_water(ComputedFeatureAllocatorHighWater {
            next_feature_id: crate::ComputedFeatureId::from_raw(
                allocator.next_feature_id.raw() + 10,
            ),
            next_corner_id: crate::ComputedFeatureCornerId::from_raw(
                allocator.next_corner_id.raw() + 10,
            ),
        })
    });

    let mut restored = base;
    let before = restored.clone();
    assert!(matches!(
        restored.rebase_after_restore(ComputedFeatureLifecycleHighWater {
            revision: ComputedFeatureRevision::from_raw(u64::MAX),
            allocator: ComputedFeatureAllocatorHighWater {
                next_feature_id: crate::ComputedFeatureId::from_raw(
                    allocator.next_feature_id.raw() + 10,
                ),
                next_corner_id: crate::ComputedFeatureCornerId::from_raw(
                    allocator.next_corner_id.raw() + 10,
                ),
            },
        }),
        Err(ComputedFeatureDocumentError::RevisionExhausted)
    ));
    assert_eq!(restored, before);
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

    let session = retained(fixture.document.clone());
    let mut original_features = ComputedFeatureDocument::new(fixture.document.id());
    original_features
        .create_fillet_set("original", 0.5, vec![original])
        .unwrap();
    let mut allocator = ComputedEvaluationAllocator::default();
    assert_eq!(
        geometry_signature(&evaluate(&session, &original_features, &mut allocator)),
        geometry_signature(&evaluate(&session, &features, &mut allocator))
    );
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
        geometry_signature(&batch_snapshot),
        geometry_signature(&sequential_snapshot)
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
    let request = first_corner_authoring_request(&fixture.document, fixture.spans);
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
fn authoring_root_work_stops_are_outcomes_not_no_root_errors() {
    let fixture = polyline_fixture();
    let session = retained(fixture.document.clone());
    let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
    let request = first_corner_authoring_request(&fixture.document, fixture.spans);

    let mut limits = OperationLimits::unlimited();
    limits.profile_subdivisions = 0;
    assert!(matches!(
        authoring
            .resolve_fillet_corner(
                request,
                0.5,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::new(CancellationToken::default(), limits),
            )
            .unwrap(),
        OperationOutcome::WorkExhausted { .. }
    ));

    let (handle, token) = cancellation_pair();
    handle.cancel();
    assert!(matches!(
        authoring
            .resolve_fillet_corner(
                request,
                0.5,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::new(token, OperationLimits::unlimited()),
            )
            .unwrap(),
        OperationOutcome::Cancelled { .. }
    ));
}

#[test]
fn invalid_feature_document_is_rejected_during_snapshot_capture() {
    let fixture = polyline_fixture();
    let session = retained(fixture.document.clone());
    let invalid = ComputedFeatureDocument::with_id(
        fixture.document.id(),
        ComputedFeatureDocumentId::from_raw(0),
    );
    assert!(matches!(
        ComputedFeatureEvaluationSnapshot::capture(
            &session,
            &invalid,
            ComputedFeatureEvaluationPolicy::default(),
        ),
        Err(ComputedFeatureSnapshotError::InvalidFeatureDocument(
            ComputedFeatureDocumentError::InvalidField {
                field: "document_id",
                ..
            }
        ))
    ));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the exact and near-parallel authoring/evaluation cases deliberately share one complete fixture"
)]
fn parallel_parents_are_typed_in_authoring_and_persistent_evaluation() {
    for (document_id, second_rise) in [(0x3000, 0.0), (0x3001, 1.0e-10)] {
        let mut document = SketchDocument::with_id(
            10.0,
            geosolve_sketch::DocumentId(geosolve_sketch::PersistentId::from_u128(document_id)),
        )
        .unwrap();
        let first_start = document.add_point("first start", [0.0, 0.0]).unwrap();
        let first_end = document.add_point("first end", [4.0, 0.0]).unwrap();
        let second_start = document.add_point("second start", [0.0, 1.0]).unwrap();
        let second_end = document
            .add_point("second end", [4.0, 1.0 + second_rise])
            .unwrap();
        let first = CurveSpan::line(
            document
                .add_curve(
                    "first",
                    CurveDefinition::Line {
                        start: first_start,
                        end: first_end,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .unwrap(),
        );
        let second = CurveSpan::line(
            document
                .add_curve(
                    "second",
                    CurveDefinition::Line {
                        start: second_start,
                        end: second_end,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .unwrap(),
        );
        let first_jet = document.evaluate_curve_jet(first, 0.5).unwrap();
        let second_jet = document.evaluate_curve_jet(second, 0.5).unwrap();
        let request = ComputedFilletCornerAuthoringRequest {
            first: ComputedFilletCurvePick {
                source: source(first),
                parameter: 0.5,
                model_position: [first_jet.position.x, first_jet.position.y],
                retained_endpoint_hint: Some(DocumentFilletTrimEndpoint::End),
            },
            second: ComputedFilletCurvePick {
                source: source(second),
                parameter: 0.5,
                model_position: [second_jet.position.x, second_jet.position.y],
                retained_endpoint_hint: Some(DocumentFilletTrimEndpoint::Start),
            },
            options: ComputedFilletAuthoringOptions::default(),
        };
        let session = retained(document.clone());
        let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
        assert!(matches!(
            authoring.resolve_fillet_corner(
                request,
                0.5,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::unlimited(),
            ),
            Err(ComputedFeatureAuthoringError::SingularParents)
        ));

        let mut features = ComputedFeatureDocument::new(document.id());
        let feature = features
            .create_fillet_set(
                "parallel",
                0.5,
                vec![NewComputedFilletCorner {
                    first: parent(
                        first,
                        0.5,
                        DocumentCurveNormalSide::Left,
                        DocumentFilletTrimEndpoint::End,
                    ),
                    second: parent(
                        second,
                        0.5,
                        DocumentCurveNormalSide::Right,
                        DocumentFilletTrimEndpoint::Start,
                    ),
                    endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
                    sweep: DocumentArcSweep::CounterClockwise,
                }],
            )
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
            ComputedFeatureEvaluationState::Failed {
                failure: ComputedFeatureFailure::SingularParents { .. }
            }
        ));
    }
}

#[test]
fn affine_non_affine_authoring_certifies_and_persists_periodic_branch_state() {
    let fixture = line_circle_fixture();
    let session = retained(fixture.document.clone());
    let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
    let resolved = complete(
        authoring
            .resolve_fillet_corner(
                fixture.request,
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
    assert_eq!(resolved.corner.first.source, source(fixture.line));
    assert_eq!(resolved.corner.second.source, source(fixture.circle));
    let mut features = ComputedFeatureDocument::new(fixture.document.id());
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
fn persistent_parent_domains_are_canonical_and_respect_visible_periodic_intervals() {
    let fixture = line_circle_fixture();
    let session = retained(fixture.document.clone());
    let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
    let resolved = complete(
        authoring
            .resolve_fillet_corner(
                fixture.request,
                0.5,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::unlimited(),
            )
            .unwrap(),
    );
    assert_eq!(resolved.corner.first.source, source(fixture.line));
    assert_eq!(resolved.corner.second.source, source(fixture.circle));

    let mut bounded_anchor = resolved.corner;
    bounded_anchor.first.periodic_anchor = Some(DocumentTrimParameter {
        parameter: 0.25,
        winding: 0,
    });
    assert!(matches!(
        evaluate_single_corner_failure(&fixture.document, 0.5, bounded_anchor),
        ComputedFeatureFailure::InvalidParentState { .. }
    ));

    let mut noncanonical_periodic = resolved.corner;
    noncanonical_periodic.second.picked_parameter += std::f64::consts::TAU;
    noncanonical_periodic.second.winding -= 1;
    assert!(matches!(
        evaluate_single_corner_failure(&fixture.document, 0.5, noncanonical_periodic),
        ComputedFeatureFailure::InvalidParentState { .. }
    ));

    let mut outside_branch = resolved.corner;
    let geosolve_sketch::ContactNeighborhood::Local { lower, upper } =
        outside_branch.second.neighborhood
    else {
        panic!("circle parent must retain a local branch cell");
    };
    outside_branch.second.neighborhood = geosolve_sketch::ContactNeighborhood::Local {
        lower: lower + std::f64::consts::TAU,
        upper: upper + std::f64::consts::TAU,
    };
    assert!(matches!(
        evaluate_single_corner_failure(&fixture.document, 0.5, outside_branch),
        ComputedFeatureFailure::InvalidParentState { .. }
    ));

    let mut trimmed = fixture.document.clone();
    trimmed
        .replace_trim_views(
            fixture.circle,
            vec![DocumentCurveTrimView {
                support: fixture.circle,
                start: DocumentTrimBoundary::Fixed(DocumentTrimParameter {
                    parameter: 0.5,
                    winding: 0,
                }),
                end: DocumentTrimBoundary::Fixed(DocumentTrimParameter {
                    parameter: 2.0,
                    winding: 0,
                }),
            }],
        )
        .unwrap();
    assert!(matches!(
        evaluate_single_corner_failure(&trimmed, 0.5, resolved.corner),
        ComputedFeatureFailure::InvalidParentState { .. }
    ));
}

#[test]
fn singular_curved_offset_remains_typed_as_offset_singularity() {
    let fixture = line_circle_fixture();
    let session = retained(fixture.document.clone());
    let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
    let mut corner = complete(
        authoring
            .resolve_fillet_corner(
                fixture.request,
                0.5,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::unlimited(),
            )
            .unwrap(),
    )
    .corner;
    assert_eq!(corner.second.source, source(fixture.circle));
    corner.second.normal_side = DocumentCurveNormalSide::Left;
    assert!(matches!(
        evaluate_single_corner_failure(&fixture.document, 1.0, corner),
        ComputedFeatureFailure::OffsetSingularity { .. }
    ));
}

#[test]
fn endpoint_claim_tolerance_is_independent_of_model_scale() {
    for (scale, document_id) in [(1.0, 0x4000), (1.0e12, 0x4001)] {
        let fixture = scaled_polyline_fixture(scale, document_id);
        let session = retained(fixture.document.clone());
        let mut features = ComputedFeatureDocument::new(fixture.document.id());
        let feature = features
            .create_fillet_set(
                "narrow surviving middle interval",
                1.999_98 * scale,
                vec![first_corner(fixture.spans), second_corner(fixture.spans)],
            )
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
        let middle = snapshot
            .source_fragment_edges(source(fixture.spans[1]))
            .next()
            .unwrap();
        let ComputedEdgeGeometry::NativeSourceFragment { interval, .. } = middle.geometry else {
            panic!("expected middle source fragment");
        };
        assert!(interval.start < interval.end);
        assert!((interval.end - interval.start - 1.0e-5).abs() < 1.0e-8);
    }
}

#[test]
fn duplicate_endpoint_claims_fail_every_participating_set() {
    let fixture = polyline_fixture();
    let session = retained(fixture.document.clone());
    let mut features = ComputedFeatureDocument::new(fixture.document.id());
    let first = features
        .create_fillet_set("first", 0.5, vec![first_corner(fixture.spans)])
        .unwrap();
    let duplicate = features
        .create_fillet_set("duplicate", 0.5, vec![first_corner(fixture.spans)])
        .unwrap();
    let mut allocator = ComputedEvaluationAllocator::default();
    let snapshot = evaluate(&session, &features, &mut allocator);
    for feature in [first, duplicate] {
        assert!(matches!(
            snapshot
                .feature_evaluations()
                .iter()
                .find(|value| value.feature == feature)
                .unwrap()
                .state,
            ComputedFeatureEvaluationState::Failed {
                failure: ComputedFeatureFailure::EndpointClaimConflict { .. }
            }
        ));
    }
    assert!(snapshot.edges().is_empty());
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
