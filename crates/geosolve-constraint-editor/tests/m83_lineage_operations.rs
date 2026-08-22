// SPDX-License-Identifier: GPL-3.0-or-later

use std::{collections::BTreeSet, sync::Arc};

use geosolve_constraint_editor::{
    FeatureAuthoringOutcome, FeatureAuthoringState, FeatureAuthoringTool,
    RetainedEditorCoordinator, SelectionItem,
};
use geosolve_sketch::{
    ContactNeighborhood, CurveCurveFilletRequest, CurveDefinition, CurveFilletParentRequest,
    CurveId, CurveSpan, DesignPointId, DocumentArcSweep, DocumentCurveNormalSide,
    DocumentDimensionDefinition, DocumentDimensionMode, DocumentEdit, DocumentElementId,
    DocumentFaceOffsetDirection, DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint,
    DocumentSolveRequest, OperationControl, OperationOutcome, RetainedSketchDocumentSession,
    SketchDocument, SolverConfig,
};
use geosolve_sketch_features::{
    ComputedCornerRef, ComputedFeatureDefinition, ComputedFeatureDocument,
};
use geosolve_sketch_lineage::{
    LineageActionDefinition, LineageDeveloperKey, LineageDocumentError, LineageInputBinding,
    LineageMaterializationMap, LineageMutation, LineageOutput, LineageOutputIdentity,
    LineageOutputIdentityFlow, LineageOutputKind, LineageOutputRef, LineagePatch,
    LineageSemanticKey, LineageSession, LineageStep, LineageStepState, VersionedActionPayload,
};
use geosolve_sketch_ops::{
    LineEndpoint, SketchOperationIdentityChange, SketchOperationKind, SketchOperationProposal,
    SketchOperationRequest, SketchOperationResult, SketchOperationSnapshot,
    SketchProfileOffsetOperand, SplitRetainedPiece, TrimRetainedSide,
};
use geosolve_sketch_topology::{
    OffsetOperandIndex, OffsetOperandRequest, PreparedOffsetOperandQuery,
};

const OPERATION_KINDS: [SketchOperationKind; 12] = [
    SketchOperationKind::Split,
    SketchOperationKind::Break,
    SketchOperationKind::Trim,
    SketchOperationKind::Extend,
    SketchOperationKind::Mirror,
    SketchOperationKind::Chamfer,
    SketchOperationKind::AssociativeFillet,
    SketchOperationKind::Rectangle,
    SketchOperationKind::RegularPolygon,
    SketchOperationKind::Slot,
    SketchOperationKind::LinearPattern,
    SketchOperationKind::ProfileOffset,
];

fn session(document: SketchDocument) -> RetainedSketchDocumentSession {
    RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("accepted operation fixture")
}

fn coordinator(document: SketchDocument) -> RetainedEditorCoordinator {
    RetainedEditorCoordinator::new(session(document)).expect("retained coordinator")
}

fn line(
    document: &mut SketchDocument,
    label: &str,
    start: [f64; 2],
    end: [f64; 2],
) -> (CurveId, [DesignPointId; 2]) {
    let points = [
        document
            .add_point(format!("{label}.start"), start)
            .expect("line start"),
        document
            .add_point(format!("{label}.end"), end)
            .expect("line end"),
    ];
    let delta = [end[0] - start[0], end[1] - start[1]];
    let length = delta[0].hypot(delta[1]);
    let curve = document
        .add_curve(
            label,
            CurveDefinition::Line {
                start: points[0],
                end: points[1],
                branch_direction: [delta[0] / length, delta[1] / length],
            },
        )
        .expect("line curve");
    (curve, points)
}

fn shared_corner_lines(document: &mut SketchDocument) -> (CurveId, CurveId) {
    let corner = document.add_point("corner", [4.0, 0.0]).expect("corner");
    let first_start = document
        .add_point("first start", [0.0, 0.0])
        .expect("first start");
    let second_end = document
        .add_point("second end", [4.0, 4.0])
        .expect("second end");
    let first = document
        .add_curve(
            "first",
            CurveDefinition::Line {
                start: first_start,
                end: corner,
                branch_direction: [1.0, 0.0],
            },
        )
        .expect("first line");
    let second = document
        .add_curve(
            "second",
            CurveDefinition::Line {
                start: corner,
                end: second_end,
                branch_direction: [0.0, 1.0],
            },
        )
        .expect("second line");
    (first, second)
}

fn proposed(
    coordinator: &RetainedEditorCoordinator,
    request: SketchOperationRequest,
) -> SketchOperationProposal {
    let outcome = SketchOperationSnapshot::capture(coordinator.session())
        .prepare(request)
        .execute(OperationControl::unlimited())
        .expect("operation preparation");
    let OperationOutcome::Completed { value, .. } = outcome else {
        panic!("unbounded operation preparation must complete");
    };
    let SketchOperationResult::Proposed(proposal) = value else {
        panic!("operation proposal expected");
    };
    *proposal
}

fn operand_index(coordinator: &RetainedEditorCoordinator) -> Arc<OffsetOperandIndex> {
    let query =
        PreparedOffsetOperandQuery::capture(coordinator.session(), OffsetOperandRequest::default())
            .expect("current topology input");
    let OperationOutcome::Completed { value, .. } = query
        .execute(OperationControl::unlimited())
        .expect("topology query")
    else {
        panic!("unbounded topology query must complete");
    };
    Arc::new(value.operand_index.expect("complete offset operand index"))
}

fn operation_fixture(
    document: SketchDocument,
    request: SketchOperationRequest,
) -> (RetainedEditorCoordinator, SketchOperationProposal) {
    let coordinator = coordinator(document);
    let proposal = proposed(&coordinator, request);
    (coordinator, proposal)
}

fn profile_offset_fixture() -> (RetainedEditorCoordinator, SketchOperationProposal) {
    let mut document = SketchDocument::new(20.0).expect("document");
    document
        .add_rectangle("source", [0.0, 0.0], 4.0, 3.0)
        .expect("source rectangle");
    let coordinator = coordinator(document);
    let index = operand_index(&coordinator);
    let face = index.faces().first().expect("rectangle face").key.clone();
    let proposal = proposed(
        &coordinator,
        SketchOperationRequest::ProfileOffset {
            label: "offset".into(),
            distance: 0.5,
            operand: SketchProfileOffsetOperand::Face {
                key: face,
                direction: DocumentFaceOffsetDirection::Outward,
            },
            operand_index: index,
        },
    );
    (coordinator, proposal)
}

#[allow(
    clippy::too_many_lines,
    reason = "the closed operation fixture dispatcher keeps each admitted request beside its exact operands"
)]
fn fixture(kind: SketchOperationKind) -> (RetainedEditorCoordinator, SketchOperationProposal) {
    match kind {
        SketchOperationKind::Split | SketchOperationKind::Break | SketchOperationKind::Trim => {
            let mut document = SketchDocument::new(20.0).expect("document");
            let (support, _) = line(&mut document, "support", [0.0, 0.0], [10.0, 0.0]);
            let support = CurveSpan::line(support);
            let request = match kind {
                SketchOperationKind::Split => SketchOperationRequest::Split {
                    support,
                    parameter: 0.5,
                    retained: SplitRetainedPiece::Before,
                },
                SketchOperationKind::Break => SketchOperationRequest::Break {
                    support,
                    start: 0.25,
                    end: 0.75,
                    retained: SplitRetainedPiece::After,
                },
                SketchOperationKind::Trim => SketchOperationRequest::Trim {
                    support,
                    parameter: 0.5,
                    retained: TrimRetainedSide::After,
                },
                _ => unreachable!("outer operation-kind match"),
            };
            operation_fixture(document, request)
        }
        SketchOperationKind::Extend => {
            let mut document = SketchDocument::new(20.0).expect("document");
            let (source, _) = line(&mut document, "source", [0.0, 0.0], [1.0, 0.0]);
            let (target, _) = line(&mut document, "target", [2.0, -1.0], [2.0, 1.0]);
            operation_fixture(
                document,
                SketchOperationRequest::ExtendLineToLine {
                    line: CurveSpan::line(source),
                    endpoint: LineEndpoint::End,
                    target: CurveSpan::line(target),
                },
            )
        }
        SketchOperationKind::Mirror => {
            let mut document = SketchDocument::new(20.0).expect("document");
            let (source, _) = line(&mut document, "source", [1.0, 0.0], [2.0, 1.0]);
            let (axis, _) = line(&mut document, "axis", [0.0, -2.0], [0.0, 2.0]);
            operation_fixture(
                document,
                SketchOperationRequest::Mirror {
                    label: "mirror".into(),
                    source,
                    axis: CurveSpan::line(axis),
                },
            )
        }
        SketchOperationKind::Chamfer => {
            let mut document = SketchDocument::new(20.0).expect("document");
            let (first, second) = shared_corner_lines(&mut document);
            operation_fixture(
                document,
                SketchOperationRequest::Chamfer {
                    label: "chamfer".into(),
                    first: CurveSpan::line(first),
                    second: CurveSpan::line(second),
                    first_distance: 1.0,
                    second_distance: 1.0,
                },
            )
        }
        SketchOperationKind::AssociativeFillet => {
            let mut document = SketchDocument::new(20.0).expect("document");
            let (first, second) = shared_corner_lines(&mut document);
            let request = CurveCurveFilletRequest {
                first: CurveFilletParentRequest {
                    curve: CurveSpan::line(first),
                    parameter: 0.75,
                    winding: 0,
                    neighborhood: ContactNeighborhood::Local {
                        lower: 0.5,
                        upper: 0.95,
                    },
                    side: DocumentCurveNormalSide::Left,
                    trim_endpoint: DocumentFilletTrimEndpoint::End,
                    periodic_anchor: None,
                },
                second: CurveFilletParentRequest {
                    curve: CurveSpan::line(second),
                    parameter: 0.25,
                    winding: 0,
                    neighborhood: ContactNeighborhood::Local {
                        lower: 0.05,
                        upper: 0.5,
                    },
                    side: DocumentCurveNormalSide::Left,
                    trim_endpoint: DocumentFilletTrimEndpoint::Start,
                    periodic_anchor: None,
                },
                endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
                sweep: DocumentArcSweep::CounterClockwise,
                radius: 1.0,
                radius_mode: DocumentDimensionMode::Driving,
            };
            operation_fixture(
                document,
                SketchOperationRequest::AssociativeFillet {
                    label: "fillet".into(),
                    request,
                },
            )
        }
        SketchOperationKind::Rectangle => operation_fixture(
            SketchDocument::new(20.0).expect("document"),
            SketchOperationRequest::Rectangle {
                label: "rectangle".into(),
                origin: [0.0, 0.0],
                width: 4.0,
                height: 3.0,
            },
        ),
        SketchOperationKind::RegularPolygon => operation_fixture(
            SketchDocument::new(20.0).expect("document"),
            SketchOperationRequest::RegularPolygon {
                label: "hexagon".into(),
                center: [0.0, 0.0],
                radius: 3.0,
                sides: 6,
                rotation: 0.25,
            },
        ),
        SketchOperationKind::Slot => operation_fixture(
            SketchDocument::new(20.0).expect("document"),
            SketchOperationRequest::Slot {
                label: "slot".into(),
                first_center: [0.0, 0.0],
                second_center: [6.0, 0.0],
                radius: 1.0,
            },
        ),
        SketchOperationKind::LinearPattern => {
            let mut document = SketchDocument::new(20.0).expect("document");
            let (source, _) = line(&mut document, "source", [0.0, 0.0], [2.0, 1.0]);
            operation_fixture(
                document,
                SketchOperationRequest::LinearPattern {
                    label: "pattern".into(),
                    sources: vec![source],
                    instances: 3,
                    step: [0.0, 3.0],
                },
            )
        }
        SketchOperationKind::ProfileOffset => profile_offset_fixture(),
    }
}

fn action_payload(step: &LineageStep) -> &geosolve_sketch_lineage::VersionedActionPayload {
    let LineageActionDefinition::Operation { action } = &step.action else {
        panic!("operation lineage action expected");
    };
    action
}

fn computed_feature_payload(step: &LineageStep) -> &VersionedActionPayload {
    let LineageActionDefinition::ComputedFeature { action } = &step.action else {
        panic!("computed-feature lineage action expected");
    };
    action
}

fn operation_source_point(
    proposal: &SketchOperationProposal,
    document: &SketchDocument,
) -> DesignPointId {
    let span = match proposal.request() {
        SketchOperationRequest::AssociativeFillet { request, .. } => request.first.curve,
        SketchOperationRequest::ProfileOffset {
            operand: SketchProfileOffsetOperand::Face { key, .. },
            ..
        } => key.outer.spans[0].span,
        _ => panic!("native Fillet/Profile Offset fixture expected"),
    };
    let curve = document.curve(span.curve).expect("native operation source");
    let CurveDefinition::Line { start, .. } = curve.definition else {
        panic!("native operation source line expected");
    };
    start
}

fn operation_target_scalar(
    kind: SketchOperationKind,
    document: &SketchDocument,
) -> geosolve_sketch::DesignScalarId {
    document
        .dimensions()
        .iter()
        .find_map(|dimension| match (&dimension.definition, kind) {
            (
                DocumentDimensionDefinition::Radius { target, .. },
                SketchOperationKind::AssociativeFillet,
            )
            | (
                DocumentDimensionDefinition::ProfileOffset { target, .. },
                SketchOperationKind::ProfileOffset,
            ) => Some(*target),
            _ => None,
        })
        .expect("native operation target scalar")
}

fn computed_fillet_fixture() -> (
    RetainedEditorCoordinator,
    [DesignPointId; 4],
    [CurveSpan; 3],
) {
    let mut document = SketchDocument::new(10.0).expect("document");
    let points = [
        document.add_point("p0", [0.0, 0.0]).expect("p0"),
        document.add_point("p1", [4.0, 0.0]).expect("p1"),
        document.add_point("p2", [4.0, 4.0]).expect("p2"),
        document.add_point("p3", [8.0, 4.0]).expect("p3"),
    ];
    let curve = document
        .add_curve(
            "three-span polyline",
            CurveDefinition::Polyline {
                points: points.to_vec(),
                closed: false,
                branch_directions: vec![[1.0, 0.0], [0.0, 1.0], [1.0, 0.0]],
            },
        )
        .expect("polyline");
    (
        coordinator(document),
        points,
        [0, 1, 2].map(|segment| CurveSpan { curve, segment }),
    )
}

fn apply_computed_fillet(
    coordinator: &mut RetainedEditorCoordinator,
    corner: DesignPointId,
) -> geosolve_sketch_features::ComputedFeatureId {
    let snapshot = coordinator
        .feature_authoring_snapshot()
        .expect("computed Fillet authoring snapshot");
    let mut authoring = FeatureAuthoringState::default();
    let candidate = match authoring.activate(
        &snapshot,
        coordinator.session().design_document(),
        FeatureAuthoringTool::Fillet,
        &[(SelectionItem::Point(corner), None)],
    ) {
        FeatureAuthoringOutcome::PreviewRequested { candidate, .. } => candidate,
        other => panic!("computed Fillet candidate expected, got {other:?}"),
    };
    let metadata = coordinator
        .prepare_feature_authoring_preview(
            coordinator.feature_document().identity(),
            &candidate,
            "M83 lineage computed Fillet",
        )
        .expect("computed Fillet preview");
    coordinator
        .apply_feature_authoring_preview(metadata.token, &candidate)
        .expect("computed Fillet publication")
        .value
}

fn raw_for_ref(
    steps: &[LineageStep],
    source: LineageOutputRef,
) -> Option<(LineageOutputKind, String)> {
    let step = steps.iter().find(|step| step.id == source.step)?;
    let output = step
        .outputs
        .iter()
        .find(|output| output.id == source.output)?;
    if let Some(reservation) = output.reservation {
        let reservation = step
            .reservations
            .iter()
            .find(|candidate| candidate.id == reservation)?;
        let (_, raw) = reservation.persistent_id.as_str().split_once(':')?;
        return Some((output.kind, raw.to_owned()));
    }
    step.output_identity(output.id)
        .and_then(LineageOutputIdentity::source)
        .and_then(|source| raw_for_ref(steps, source))
}

fn element_key(element: DocumentElementId) -> Option<(LineageOutputKind, String)> {
    let kind = match element {
        DocumentElementId::Point(_) => LineageOutputKind::Point,
        DocumentElementId::Scalar(_) => LineageOutputKind::Scalar,
        DocumentElementId::Curve(_) => LineageOutputKind::Curve,
        DocumentElementId::Contact(_) => LineageOutputKind::Contact,
        DocumentElementId::Constraint(_) => LineageOutputKind::Constraint,
        DocumentElementId::Dimension(_) => LineageOutputKind::Dimension,
        DocumentElementId::Parameter(_) => LineageOutputKind::Parameter,
        DocumentElementId::ExternalBinding(_) => LineageOutputKind::ExternalBinding,
        DocumentElementId::Source(_) => LineageOutputKind::Source,
        _ => return None,
    };
    Some((kind, element.persistent_id().to_string()))
}

fn assert_cold_matches(coordinator: &RetainedEditorCoordinator) {
    let session = coordinator
        .lineage_session_json()
        .expect("canonical lineage session");
    let ledger = coordinator
        .lineage_host_input_ledger_json()
        .expect("canonical lineage host-input ledger");
    let lineage = LineageSession::from_session_json(&session).expect("strict lineage session");
    assert_eq!(coordinator.lineage_identity(), lineage.identity());
    assert_eq!(
        coordinator.history_len(),
        lineage.undo_len() + 1 + lineage.redo_len(),
        "the public history length must be derived from lineage"
    );
    assert_eq!(coordinator.history_cursor(), lineage.undo_len());
    assert_eq!(coordinator.can_undo(), lineage.can_undo());
    assert_eq!(coordinator.can_redo(), lineage.can_redo());
    let cold = RetainedEditorCoordinator::lineage_materialization_checkpoint(&session)
        .expect("cold operation materialization");
    let live = coordinator.checkpoint();
    assert_eq!(cold.design_json(), live.design_json());
    assert_eq!(cold.feature_json(), live.feature_json());
    let accepted = if live.accepted_belongs_to_current_design() {
        RetainedEditorCoordinator::lineage_cold_current_accepted_evidence_checkpoint(
            &session,
            Some(&ledger),
            coordinator.session().parameter_batch(),
            coordinator.session().external_snapshot_set(),
        )
        .expect("cold current operation accepted authority")
    } else {
        RetainedEditorCoordinator::lineage_cold_historical_accepted_evidence_checkpoint(
            &session,
            Some(&ledger),
            coordinator.session().accepted_parameter_batch(),
            coordinator.session().accepted_external_snapshot_set(),
        )
        .expect("cold historical operation accepted authority")
        .expect("retained historical operation authority")
    };
    assert_eq!(
        accepted.accepted_uses_draft_v5(),
        live.accepted_uses_draft_v5(),
        "accepted operation encoding must agree with strict-cold authority"
    );
    let mut canonical = if accepted.accepted_uses_draft_v5() {
        SketchDocument::from_draft_v5_json(
            accepted
                .accepted_json()
                .expect("cold accepted operation document"),
        )
    } else {
        SketchDocument::from_json(
            accepted
                .accepted_json()
                .expect("cold accepted operation document"),
        )
    }
    .expect("decode cold accepted operation document");
    canonical
        .retain_persistent_identity_high_water(
            coordinator.session().persistent_identity_high_water(),
        )
        .expect("merge operation lifecycle high-water");
    let canonical = if accepted.accepted_uses_draft_v5() {
        canonical.to_draft_v5_json()
    } else {
        canonical.to_canonical_json()
    }
    .expect("encode cold accepted operation document");
    assert_eq!(
        Some(canonical.as_str()),
        live.accepted_json(),
        "live operation geometry must equal strict-cold authority modulo monotonic lifecycle high-water"
    );
    assert_eq!(
        accepted.accepted_belongs_to_current_design(),
        live.accepted_belongs_to_current_design()
    );
    let map = RetainedEditorCoordinator::lineage_materialization_map_json_for_session(&session)
        .expect("operation ownership map");
    RetainedEditorCoordinator::validate_lineage_materialization_map_json(&session, &map)
        .expect("authenticated operation ownership map");
}

fn logical_step(
    session: &LineageSession,
    key: &str,
    schema: &str,
    inputs: Vec<LineageInputBinding>,
    output_kind: LineageOutputKind,
) -> LineageStep {
    let allocators = session.document().allocator_high_water();
    let mut action = VersionedActionPayload::empty(LineageSemanticKey::new(schema).unwrap(), 1);
    action.inputs = inputs;
    LineageStep::new(
        allocators.next_step_id,
        LineageDeveloperKey::new(key).unwrap(),
        key,
        LineageActionDefinition::Operation { action },
        vec![LineageOutput {
            id: allocators.next_output_id,
            key: LineageSemanticKey::new("result").unwrap(),
            kind: output_kind,
            reservation: None,
        }],
        Vec::new(),
    )
}

fn output_ref_for_raw(
    coordinator: &RetainedEditorCoordinator,
    owner: Option<geosolve_sketch_lineage::LineageStepId>,
    kind: LineageOutputKind,
    raw: &str,
) -> LineageOutputRef {
    let steps = coordinator.lineage_document().steps();
    for step in steps {
        if owner.is_some_and(|owner| step.id != owner) {
            continue;
        }
        for output in &step.outputs {
            let reference = LineageOutputRef {
                document: coordinator.lineage_document().id(),
                step: step.id,
                output: output.id,
                kind: output.kind,
            };
            if output.kind == kind
                && raw_for_ref(steps, reference)
                    .is_some_and(|candidate| candidate.0 == kind && candidate.1 == raw)
            {
                return reference;
            }
        }
    }
    panic!("missing {kind:?} lineage port for {raw}");
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the closed twelve-operation matrix keeps typed request and identity evidence row-local"
)]
fn m83_w4_all_native_operations_publish_exact_typed_lineage_identity() {
    for kind in OPERATION_KINDS {
        let (mut coordinator, proposal) = fixture(kind);
        assert_eq!(proposal.request().kind(), kind);
        let expected = proposal.expected_application().clone();
        let outcome = coordinator
            .apply_sketch_operation(&proposal)
            .expect("lineage operation publication");
        assert!(outcome.published_accepted.is_some(), "{kind:?}");
        assert_eq!(coordinator.lineage_document().steps().len(), 2, "{kind:?}");
        let steps = coordinator.lineage_document().steps();
        let step = &steps[1];
        let action = action_payload(step);
        assert_eq!(
            action.schema.as_str(),
            format!("geosolve.operation.v1.{}", kind.semantic_key())
        );
        assert!(action.parameters.contains_key("authored_intent"));
        assert!(
            step.outputs.iter().any(|output| {
                output.kind == LineageOutputKind::Operation
                    && matches!(
                        step.output_identity(output.id)
                            .map(|identity| identity.flow),
                        Some(LineageOutputIdentityFlow::OwnedLogical)
                    )
            }),
            "stable operation port for {kind:?}"
        );

        let created = step
            .output_identities
            .iter()
            .filter(|identity| matches!(identity.flow, LineageOutputIdentityFlow::Created { .. }))
            .map(|identity| {
                let output = step
                    .outputs
                    .iter()
                    .find(|output| output.id == identity.output)
                    .expect("created output");
                raw_for_ref(
                    steps,
                    LineageOutputRef {
                        document: coordinator.lineage_document().id(),
                        step: step.id,
                        output: output.id,
                        kind: output.kind,
                    },
                )
                .expect("created native identity")
            })
            .collect::<Vec<_>>();
        let continued = step
            .output_identities
            .iter()
            .filter_map(|identity| match identity.flow {
                LineageOutputIdentityFlow::Continued { source } => raw_for_ref(steps, source),
                _ => None,
            })
            .collect::<Vec<_>>();
        let input_native = action
            .inputs
            .iter()
            .filter_map(|input| raw_for_ref(steps, input.source))
            .collect::<Vec<_>>();

        for change in &expected.identity_changes {
            match change {
                SketchOperationIdentityChange::Proposed(element) => {
                    if let Some(key) = element_key(*element) {
                        assert!(
                            created.contains(&key),
                            "created {key:?} for {kind:?}; actual {created:?}"
                        );
                    }
                }
                SketchOperationIdentityChange::Replaced(element) => {
                    if let Some(key) = element_key(*element) {
                        assert!(continued.contains(&key), "continued {key:?} for {kind:?}");
                    }
                }
                SketchOperationIdentityChange::Split { source, .. } => {
                    assert!(
                        continued.contains(&(LineageOutputKind::Curve, source.to_string())),
                        "continued split support for {kind:?}"
                    );
                }
                SketchOperationIdentityChange::Retained(element) => {
                    if let Some(key) = element_key(*element) {
                        assert!(
                            input_native.contains(&key) || continued.contains(&key),
                            "typed retained input {key:?} for {kind:?}"
                        );
                    }
                }
                _ => panic!("unmapped operation identity change"),
            }
        }
        if matches!(
            kind,
            SketchOperationKind::Split | SketchOperationKind::Break | SketchOperationKind::Trim
        ) {
            assert!(
                continued
                    .iter()
                    .any(|(kind, _)| *kind == LineageOutputKind::Curve)
            );
        } else {
            assert!(
                !created.is_empty() || !continued.is_empty(),
                "identity evidence for {kind:?}"
            );
        }
        assert_cold_matches(&coordinator);
    }
}

#[test]
fn m83_w11_all_twelve_native_operation_routes_use_lineage_history() {
    assert_eq!(OPERATION_KINDS.len(), 12);
    assert_eq!(
        OPERATION_KINDS
            .iter()
            .map(|kind| kind.semantic_key())
            .collect::<BTreeSet<_>>()
            .len(),
        OPERATION_KINDS.len(),
        "the operation mutation inventory must remain closed and unique"
    );

    for kind in OPERATION_KINDS {
        let (mut coordinator, proposal) = fixture(kind);
        let before = coordinator.lineage_identity();
        let before_steps = coordinator.lineage_document().steps().to_vec();
        let before_history = (coordinator.history_len(), coordinator.history_cursor());
        coordinator
            .apply_sketch_operation(&proposal)
            .expect("native operation lineage route");
        let applied = coordinator.lineage_identity();
        let applied_steps = coordinator.lineage_document().steps().to_vec();
        assert_ne!(applied, before, "{kind:?} must mutate lineage authority");
        assert_eq!(
            (coordinator.history_len(), coordinator.history_cursor()),
            (before_history.0 + 1, before_history.1 + 1),
            "{kind:?} must add exactly one lineage history position"
        );
        assert!(!coordinator.can_redo(), "{kind:?} must clear lineage Redo");
        assert_cold_matches(&coordinator);

        coordinator.undo().expect("Undo native operation");
        let undone = coordinator.lineage_identity();
        assert_eq!(undone.document, before.document, "Undo {kind:?}");
        assert!(
            undone.revision > applied.revision,
            "Undo {kind:?} must restore intent at a fresh never-reused revision"
        );
        assert_eq!(
            coordinator.lineage_document().steps(),
            before_steps,
            "Undo {kind:?} must restore the exact declarative program"
        );
        assert_eq!(coordinator.history_cursor(), before_history.1);
        assert!(coordinator.can_redo());
        assert_cold_matches(&coordinator);

        coordinator.redo().expect("Redo native operation");
        let redone = coordinator.lineage_identity();
        assert_eq!(redone.document, applied.document, "Redo {kind:?}");
        assert!(
            redone.revision > undone.revision,
            "Redo {kind:?} must restore intent at a fresh never-reused revision"
        );
        assert_eq!(
            coordinator.lineage_document().steps(),
            applied_steps,
            "Redo {kind:?} must restore the exact declarative program"
        );
        assert_eq!(coordinator.history_cursor(), before_history.1 + 1);
        assert_cold_matches(&coordinator);
    }
}

#[test]
fn m83_w11_computed_fillet_publication_edit_delete_and_history_use_lineage() {
    let (mut coordinator, points, _) = computed_fillet_fixture();
    let baseline = coordinator.lineage_identity();
    let baseline_steps = coordinator.lineage_document().steps().to_vec();
    let baseline_features = coordinator.feature_document().features().to_vec();
    let feature = apply_computed_fillet(&mut coordinator, points[1]);
    let published = coordinator.lineage_identity();
    let published_steps = coordinator.lineage_document().steps().to_vec();
    let published_features = coordinator.feature_document().features().to_vec();
    assert_ne!(published, baseline);
    assert_eq!(coordinator.history_len(), 2);
    assert_eq!(coordinator.history_cursor(), 1);
    assert_cold_matches(&coordinator);

    coordinator
        .set_computed_fillet_radius(coordinator.feature_document().identity(), feature, 0.75)
        .expect("computed Fillet parameter route");
    let edited = coordinator.lineage_identity();
    let edited_steps = coordinator.lineage_document().steps().to_vec();
    let edited_features = coordinator.feature_document().features().to_vec();
    assert_ne!(edited, published);
    assert_eq!(coordinator.history_len(), 3);
    assert_eq!(coordinator.history_cursor(), 2);
    assert_cold_matches(&coordinator);

    coordinator.set_selection([SelectionItem::Feature(feature)]);
    coordinator
        .delete_selected(coordinator.session().design_identity())
        .expect("computed Fillet delete route");
    let deleted = coordinator.lineage_identity();
    let deleted_steps = coordinator.lineage_document().steps().to_vec();
    let deleted_features = coordinator.feature_document().features().to_vec();
    assert_ne!(deleted, edited);
    assert!(coordinator.feature_document().feature(feature).is_none());
    assert_eq!(coordinator.history_len(), 4);
    assert_eq!(coordinator.history_cursor(), 3);
    assert_cold_matches(&coordinator);

    coordinator.undo().expect("Undo computed Fillet deletion");
    assert_eq!(coordinator.lineage_document().steps(), edited_steps);
    assert_eq!(coordinator.feature_document().features(), edited_features);
    assert!(coordinator.feature_document().feature(feature).is_some());
    assert_cold_matches(&coordinator);

    coordinator
        .undo()
        .expect("Undo computed Fillet radius edit");
    assert_eq!(coordinator.lineage_document().steps(), published_steps);
    assert_eq!(
        coordinator.feature_document().features(),
        published_features
    );
    assert_cold_matches(&coordinator);

    coordinator
        .undo()
        .expect("Undo computed Fillet publication");
    assert_eq!(coordinator.lineage_document().steps(), baseline_steps);
    assert_eq!(coordinator.feature_document().features(), baseline_features);
    assert!(coordinator.feature_document().feature(feature).is_none());
    assert_eq!(coordinator.history_cursor(), 0);
    assert_cold_matches(&coordinator);

    coordinator
        .redo()
        .expect("Redo computed Fillet publication");
    assert_eq!(coordinator.lineage_document().steps(), published_steps);
    assert_eq!(
        coordinator.feature_document().features(),
        published_features
    );
    assert_cold_matches(&coordinator);

    coordinator
        .redo()
        .expect("Redo computed Fillet radius edit");
    assert_eq!(coordinator.lineage_document().steps(), edited_steps);
    assert_eq!(coordinator.feature_document().features(), edited_features);
    assert_cold_matches(&coordinator);

    coordinator.redo().expect("Redo computed Fillet deletion");
    assert_eq!(coordinator.lineage_document().steps(), deleted_steps);
    assert_eq!(coordinator.feature_document().features(), deleted_features);
    assert!(coordinator.feature_document().feature(feature).is_none());
    assert_eq!(coordinator.history_cursor(), 3);
    assert_cold_matches(&coordinator);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one owner lifecycle regression keeps dependency blocking, rebind, retirement and ordering evidence exact"
)]
fn m83_w4_operation_owner_delete_rebind_and_ordering_keep_exact_identity() {
    let (mut coordinator, proposal) = fixture(SketchOperationKind::Mirror);
    let source_curve = match proposal.request() {
        SketchOperationRequest::Mirror { source, .. } => *source,
        _ => unreachable!("mirror fixture"),
    };
    coordinator
        .apply_sketch_operation(&proposal)
        .expect("mirror lineage publication");
    let owner = coordinator.lineage_document().steps()[1].id;
    let owner_outputs = coordinator.lineage_document().steps()[1].outputs.clone();
    let owner_identities = coordinator.lineage_document().steps()[1]
        .output_identities
        .clone();
    let owner_reservations = coordinator.lineage_document().steps()[1]
        .reservations
        .clone();
    let created_curve = coordinator.lineage_document().steps()[1]
        .output_identities
        .iter()
        .find_map(|identity| {
            if !matches!(identity.flow, LineageOutputIdentityFlow::Created { .. }) {
                return None;
            }
            coordinator.lineage_document().steps()[1]
                .outputs
                .iter()
                .find(|output| {
                    output.id == identity.output && output.kind == LineageOutputKind::Curve
                })
                .map(|output| LineageOutputRef {
                    document: coordinator.lineage_document().id(),
                    step: owner,
                    output: output.id,
                    kind: output.kind,
                })
        })
        .expect("mirrored curve output");
    let baseline_curve = output_ref_for_raw(
        &coordinator,
        Some(coordinator.lineage_document().steps()[0].id),
        LineageOutputKind::Curve,
        &source_curve.to_string(),
    );
    let canonical = coordinator
        .lineage_session_json()
        .expect("canonical operation lineage");

    let mut dependent = LineageSession::from_session_json(&canonical).expect("dependent session");
    let consumer = logical_step(
        &dependent,
        "operation-consumer",
        "geosolve.operation.v1.consumer",
        vec![LineageInputBinding {
            key: LineageSemanticKey::new("source").unwrap(),
            kind: LineageOutputKind::Curve,
            source: created_curve,
        }],
        LineageOutputKind::Collection,
    );
    let consumer_id = consumer.id;
    dependent
        .apply_patch(LineagePatch::new(
            dependent.identity(),
            vec![LineageMutation::Insert {
                before: None,
                step: Box::new(consumer),
            }],
        ))
        .expect("dependent insertion");
    let blocked = dependent
        .apply_patch(LineagePatch::new(
            dependent.identity(),
            vec![LineageMutation::Tombstone { step: owner }],
        ))
        .expect_err("live dependent must block whole-step deletion");
    assert!(matches!(
        blocked,
        LineageDocumentError::LiveDependent {
            deleted,
            dependent
        } if deleted == owner && dependent == consumer_id
    ));
    dependent
        .apply_patch(LineagePatch::new(
            dependent.identity(),
            vec![
                LineageMutation::Rebind {
                    step: consumer_id,
                    input: LineageSemanticKey::new("source").unwrap(),
                    target: baseline_curve,
                },
                LineageMutation::Tombstone { step: owner },
            ],
        ))
        .expect("explicit typed rebind and whole-step delete");
    let deleted = dependent
        .document()
        .step(owner)
        .expect("retained tombstone");
    assert_eq!(deleted.state, LineageStepState::Tombstoned);
    assert_eq!(deleted.outputs, owner_outputs);
    assert_eq!(deleted.output_identities, owner_identities);
    assert_eq!(deleted.reservations, owner_reservations);
    assert!(
        LineageMaterializationMap::derive(dependent.document())
            .expect("deleted owner map")
            .live_owned_outputs(owner)
            .is_empty()
    );
    dependent
        .undo()
        .expect("undo result")
        .expect("undo deletion");
    assert_eq!(
        dependent
            .document()
            .step(owner)
            .expect("restored owner")
            .state,
        LineageStepState::Live
    );
    assert_eq!(
        dependent.document().step(owner).unwrap().outputs,
        owner_outputs
    );
    dependent
        .redo()
        .expect("redo result")
        .expect("redo deletion");
    assert_eq!(
        dependent
            .document()
            .step(owner)
            .expect("deleted owner")
            .state,
        LineageStepState::Tombstoned
    );

    let mut ordered = LineageSession::from_session_json(&canonical).expect("ordering session");
    let unrelated = logical_step(
        &ordered,
        "unrelated-operation",
        "geosolve.operation.v1.unrelated",
        Vec::new(),
        LineageOutputKind::Collection,
    );
    let unrelated_id = unrelated.id;
    ordered
        .apply_patch(LineagePatch::new(
            ordered.identity(),
            vec![LineageMutation::Insert {
                before: Some(owner),
                step: Box::new(unrelated),
            }],
        ))
        .expect("earlier unrelated insertion");
    let operation = ordered
        .document()
        .step(owner)
        .expect("stable operation owner");
    assert_eq!(operation.outputs, owner_outputs);
    assert_eq!(operation.output_identities, owner_identities);
    assert_eq!(operation.reservations, owner_reservations);
    ordered
        .apply_patch(LineagePatch::new(
            ordered.identity(),
            vec![LineageMutation::Reorder {
                step: unrelated_id,
                before: None,
            }],
        ))
        .expect("unrelated reorder");
    let operation = ordered
        .document()
        .step(owner)
        .expect("stable reordered owner");
    assert_eq!(operation.outputs, owner_outputs);
    assert_eq!(operation.output_identities, owner_identities);
    assert_eq!(operation.reservations, owner_reservations);

    let mut retirement = LineageSession::from_session_json(&canonical).expect("retirement session");
    let allocators = retirement.document().allocator_high_water();
    let mut action = VersionedActionPayload::empty(
        LineageSemanticKey::new("geosolve.operation.v1.retire-output").unwrap(),
        1,
    );
    action.inputs.push(LineageInputBinding {
        key: LineageSemanticKey::new("source").unwrap(),
        kind: LineageOutputKind::Curve,
        source: created_curve,
    });
    let retirement_step = LineageStep::new(
        allocators.next_step_id,
        LineageDeveloperKey::new("retire-created-curve").unwrap(),
        "Retire created curve",
        LineageActionDefinition::Operation { action },
        vec![LineageOutput {
            id: allocators.next_output_id,
            key: LineageSemanticKey::new("retired-curve").unwrap(),
            kind: LineageOutputKind::Curve,
            reservation: None,
        }],
        Vec::new(),
    )
    .with_output_identities(vec![LineageOutputIdentity {
        output: allocators.next_output_id,
        flow: LineageOutputIdentityFlow::Retired {
            source: created_curve,
        },
    }]);
    retirement
        .apply_patch(LineagePatch::new(
            retirement.identity(),
            vec![LineageMutation::Insert {
                before: None,
                step: Box::new(retirement_step),
            }],
        ))
        .expect("explicit retirement evidence");
    let stale_consumer = logical_step(
        &retirement,
        "retired-output-consumer",
        "geosolve.operation.v1.retired-consumer",
        vec![LineageInputBinding {
            key: LineageSemanticKey::new("source").unwrap(),
            kind: LineageOutputKind::Curve,
            source: created_curve,
        }],
        LineageOutputKind::Collection,
    );
    let stale_id = stale_consumer.id;
    let rejected = retirement
        .apply_patch(LineagePatch::new(
            retirement.identity(),
            vec![LineageMutation::Insert {
                before: None,
                step: Box::new(stale_consumer),
            }],
        ))
        .expect_err("retired native identity cannot be rebound implicitly");
    assert!(matches!(
        rejected,
        LineageDocumentError::RetiredOutputReference { step, .. } if step == stale_id
    ));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the two native W5 families share one exact owner/source/parameter/delete lifecycle contract"
)]
fn m83_w5_native_fillet_and_profile_offset_keep_intent_identity_across_edits() {
    for kind in [
        SketchOperationKind::AssociativeFillet,
        SketchOperationKind::ProfileOffset,
    ] {
        let (mut coordinator, proposal) = fixture(kind);
        coordinator
            .apply_sketch_operation(&proposal)
            .expect("native lineage operation publication");
        let owner = coordinator.lineage_document().steps()[1].id;
        let initial_step = coordinator.lineage_document().steps()[1].clone();
        let initial_action = initial_step.action.clone();
        let source = operation_source_point(&proposal, coordinator.session().design_document());
        let source_position = coordinator
            .session()
            .design_document()
            .point(source)
            .expect("source point")
            .position;

        assert!(initial_step.outputs.iter().any(|output| {
            output.kind == LineageOutputKind::Parameter
                && matches!(
                    initial_step
                        .output_identity(output.id)
                        .map(|identity| identity.flow),
                    Some(LineageOutputIdentityFlow::OwnedLogical)
                )
        }));
        match kind {
            SketchOperationKind::AssociativeFillet => {
                assert!(
                    action_payload(&initial_step)
                        .inputs
                        .iter()
                        .any(|input| input.key.as_str() == "first")
                );
                assert!(
                    action_payload(&initial_step)
                        .inputs
                        .iter()
                        .any(|input| input.key.as_str() == "second")
                );
            }
            SketchOperationKind::ProfileOffset => {
                assert!(initial_step.outputs.iter().any(|output| {
                    output.kind == LineageOutputKind::Profile
                        && matches!(
                            initial_step
                                .output_identity(output.id)
                                .map(|identity| identity.flow),
                            Some(LineageOutputIdentityFlow::OwnedLogical)
                        )
                }));
            }
            _ => unreachable!("closed W5 native family loop"),
        }

        coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::SetPointPosition {
                    point: source,
                    position: [source_position[0] - 0.25, source_position[1]],
                },
            )
            .expect("native operation source rewrite");
        let after_source = coordinator
            .lineage_document()
            .step(owner)
            .expect("unchanged dependent operation owner")
            .clone();
        assert_eq!(
            after_source.action, initial_action,
            "source rewrite must leave {kind:?} intent bytes unchanged"
        );
        assert_eq!(after_source.outputs, initial_step.outputs);
        assert_eq!(
            after_source.output_identities,
            initial_step.output_identities
        );
        assert_eq!(after_source.reservations, initial_step.reservations);

        let target_id = operation_target_scalar(kind, coordinator.session().design_document());
        let replacement = coordinator
            .session()
            .design_document()
            .scalar(target_id)
            .expect("native operation target")
            .value
            * 0.75;
        let before_parameter_action = after_source.action.clone();
        coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::SetScalarValue {
                    scalar: target_id,
                    value: replacement,
                },
            )
            .expect("native operation parameter rewrite");
        let after_parameter = coordinator
            .lineage_document()
            .step(owner)
            .expect("rewritten native operation owner")
            .clone();
        assert_ne!(after_parameter.action, before_parameter_action);
        assert_eq!(after_parameter.outputs, initial_step.outputs);
        assert_eq!(
            after_parameter.output_identities,
            initial_step.output_identities
        );
        assert_eq!(after_parameter.reservations, initial_step.reservations);
        assert_eq!(coordinator.lineage_document().steps().len(), 2);
        assert_cold_matches(&coordinator);

        coordinator.undo().expect("undo native parameter rewrite");
        assert_eq!(
            coordinator
                .lineage_document()
                .step(owner)
                .expect("undone native operation owner")
                .action,
            before_parameter_action
        );
        coordinator.redo().expect("redo native parameter rewrite");
        assert_eq!(
            coordinator
                .lineage_document()
                .step(owner)
                .expect("redone native operation owner")
                .action,
            after_parameter.action
        );
        assert_cold_matches(&coordinator);

        let canonical = coordinator
            .lineage_session_json()
            .expect("native operation lineage session");
        let mut deleted = LineageSession::from_session_json(&canonical).expect("delete session");
        deleted
            .apply_patch(LineagePatch::new(
                deleted.identity(),
                vec![LineageMutation::Tombstone { step: owner }],
            ))
            .expect("whole native operation deletion");
        assert!(
            LineageMaterializationMap::derive(deleted.document())
                .expect("deleted native operation map")
                .live_owned_outputs(owner)
                .is_empty()
        );
        let deleted_cold = RetainedEditorCoordinator::lineage_materialization_checkpoint(
            &deleted
                .to_canonical_session_json()
                .expect("deleted native operation session"),
        )
        .expect("cold deleted native operation");
        for identity in &initial_step.output_identities {
            if !matches!(identity.flow, LineageOutputIdentityFlow::Created { .. }) {
                continue;
            }
            let output = initial_step
                .outputs
                .iter()
                .find(|output| output.id == identity.output)
                .expect("created native output");
            let reference = LineageOutputRef {
                document: coordinator.lineage_document().id(),
                step: owner,
                output: output.id,
                kind: output.kind,
            };
            let (_, raw) = raw_for_ref(coordinator.lineage_document().steps(), reference)
                .expect("created native raw identity");
            assert!(
                !deleted_cold.design_json().contains(&format!("\"{raw}\"")),
                "deleted {kind:?} still materialized native identity {raw}"
            );
        }
        deleted
            .undo()
            .expect("native delete Undo")
            .expect("Undo entry");
        assert_eq!(
            deleted
                .document()
                .step(owner)
                .expect("restored native owner"),
            &after_parameter
        );
        deleted
            .redo()
            .expect("native delete Redo")
            .expect("Redo entry");
        assert_eq!(
            deleted
                .document()
                .step(owner)
                .expect("deleted native owner")
                .state,
            LineageStepState::Tombstoned
        );
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one computed-Fillet lifecycle keeps stable feature/corner ownership distinct from revision-local evaluated fragments"
)]
fn m83_w5_computed_fillet_keeps_feature_corner_and_radius_lineage_only() {
    let (mut coordinator, points, spans) = computed_fillet_fixture();
    let feature = apply_computed_fillet(&mut coordinator, points[1]);
    let owner = coordinator.lineage_document().steps()[1].id;
    let initial_step = coordinator.lineage_document().steps()[1].clone();
    let initial_payload = computed_feature_payload(&initial_step);
    assert_eq!(
        initial_payload.schema.as_str(),
        "geosolve.feature.v1.fillet-set"
    );
    assert!(initial_payload.inputs.iter().any(|input| {
        raw_for_ref(coordinator.lineage_document().steps(), input.source).is_some_and(
            |(kind, raw)| kind == LineageOutputKind::Curve && raw == spans[0].curve.to_string(),
        )
    }));
    assert!(initial_payload.inputs.iter().any(|input| {
        raw_for_ref(coordinator.lineage_document().steps(), input.source).is_some_and(
            |(kind, raw)| kind == LineageOutputKind::Curve && raw == spans[1].curve.to_string(),
        )
    }));

    let feature_record = coordinator
        .feature_document()
        .feature(feature)
        .expect("computed Fillet feature");
    let ComputedFeatureDefinition::FilletSet(fillet) = &feature_record.definition;
    assert_eq!(fillet.corners.len(), 1);
    let corner = fillet.corners[0].id;
    let stable_raw = initial_step
        .outputs
        .iter()
        .filter_map(|output| {
            let reference = LineageOutputRef {
                document: coordinator.lineage_document().id(),
                step: owner,
                output: output.id,
                kind: output.kind,
            };
            raw_for_ref(coordinator.lineage_document().steps(), reference)
        })
        .collect::<BTreeSet<_>>();
    assert!(stable_raw.contains(&(LineageOutputKind::Feature, feature.to_string())));
    assert!(stable_raw.contains(&(LineageOutputKind::FeatureCorner, corner.to_string())));
    for (kind, raw) in [
        (LineageOutputKind::Feature, feature.to_string()),
        (LineageOutputKind::FeatureCorner, corner.to_string()),
    ] {
        let output = initial_step
            .outputs
            .iter()
            .find(|output| {
                output.kind == kind
                    && raw_for_ref(
                        coordinator.lineage_document().steps(),
                        LineageOutputRef {
                            document: coordinator.lineage_document().id(),
                            step: owner,
                            output: output.id,
                            kind: output.kind,
                        },
                    )
                    .is_some_and(|candidate| candidate.0 == kind && candidate.1 == raw)
            })
            .expect("stable computed feature/corner output");
        assert!(matches!(
            initial_step
                .output_identity(output.id)
                .map(|identity| identity.flow),
            Some(LineageOutputIdentityFlow::Created { .. })
        ));
    }
    assert!(initial_step.outputs.iter().any(|output| {
        output.kind == LineageOutputKind::Parameter
            && matches!(
                initial_step
                    .output_identity(output.id)
                    .map(|identity| identity.flow),
                Some(LineageOutputIdentityFlow::OwnedLogical)
            )
    }));
    for output in initial_step.outputs.iter().filter(|output| {
        matches!(
            output.kind,
            LineageOutputKind::Curve | LineageOutputKind::CurveSpan | LineageOutputKind::TrimView
        )
    }) {
        assert!(
            matches!(
                initial_step
                    .output_identity(output.id)
                    .map(|identity| identity.flow),
                Some(
                    LineageOutputIdentityFlow::Aliased { .. }
                        | LineageOutputIdentityFlow::Continued { .. }
                )
            ),
            "computed Fillet claimed stable generated geometry: {output:?} / {:?}",
            initial_step.output_identity(output.id)
        );
    }
    assert!(
        initial_step
            .reservations
            .iter()
            .all(|reservation| !matches!(
                reservation.kind,
                geosolve_sketch_lineage::LineageReservationKind::Curve
                    | geosolve_sketch_lineage::LineageReservationKind::CurveSpan
                    | geosolve_sketch_lineage::LineageReservationKind::TrimView
            ))
    );
    let evaluated_arc = coordinator
        .computed_snapshot()
        .expect("computed Fillet output")
        .fillet_arc_edge(ComputedCornerRef { feature, corner })
        .expect("revision-local generated Fillet arc")
        .id;

    let initial_action = initial_step.action.clone();
    let p0 = coordinator
        .session()
        .design_document()
        .point(points[0])
        .expect("source point")
        .position;
    coordinator
        .apply_edit(
            coordinator.session().design_identity(),
            DocumentEdit::SetPointPosition {
                point: points[0],
                position: [p0[0] - 0.25, p0[1]],
            },
        )
        .expect("computed Fillet source rewrite");
    let after_source = coordinator
        .lineage_document()
        .step(owner)
        .expect("unchanged computed feature owner")
        .clone();
    assert_ne!(
        after_source.action, initial_action,
        "accepted source movement durably refreshes explicit Fillet contact continuation"
    );
    let refreshed_intent = computed_feature_payload(&after_source)
        .parameters
        .get("authored_intent")
        .expect("refreshed authored intent");
    let initial_intent = initial_payload
        .parameters
        .get("authored_intent")
        .expect("initial authored intent");
    assert_eq!(
        refreshed_intent.get("body"),
        initial_intent.get("body"),
        "source refresh must not rewrite the user's genesis feature recipe"
    );
    assert_eq!(
        refreshed_intent.get("owned_output_field_manifest"),
        initial_intent.get("owned_output_field_manifest"),
        "source refresh must not change the feature's declared writable surface"
    );
    assert_eq!(after_source.outputs, initial_step.outputs);
    assert_eq!(
        after_source.output_identities,
        initial_step.output_identities
    );
    assert_eq!(after_source.reservations, initial_step.reservations);
    let refreshed_arc = coordinator
        .computed_snapshot()
        .expect("refreshed computed output")
        .fillet_arc_edge(ComputedCornerRef { feature, corner })
        .expect("refreshed revision-local Fillet arc")
        .id;
    assert_ne!(
        refreshed_arc.evaluation, evaluated_arc.evaluation,
        "evaluated arc identity must remain revision-local"
    );

    let exact = coordinator
        .computed_evaluation_input()
        .expect("current computed input");
    coordinator
        .set_computed_fillet_radius_exact(exact, feature, 0.75)
        .expect("computed Fillet radius rewrite");
    let after_radius = coordinator
        .lineage_document()
        .step(owner)
        .expect("rewritten computed feature owner")
        .clone();
    assert_ne!(after_radius.action, after_source.action);
    assert_eq!(after_radius.outputs, initial_step.outputs);
    assert_eq!(
        after_radius.output_identities,
        initial_step.output_identities
    );
    assert_eq!(after_radius.reservations, initial_step.reservations);
    let ComputedFeatureDefinition::FilletSet(updated) = &coordinator
        .feature_document()
        .feature(feature)
        .expect("updated computed Fillet")
        .definition;
    assert_eq!(updated.radius.to_bits(), 0.75_f64.to_bits());
    assert_eq!(updated.corners[0].id, corner);
    assert_cold_matches(&coordinator);

    coordinator.undo().expect("Undo computed radius rewrite");
    assert_eq!(
        coordinator
            .lineage_document()
            .step(owner)
            .expect("undone computed owner")
            .action,
        after_source.action
    );
    coordinator.redo().expect("Redo computed radius rewrite");
    assert_eq!(
        coordinator
            .lineage_document()
            .step(owner)
            .expect("redone computed owner")
            .action,
        after_radius.action
    );

    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Feature(feature)]);
    coordinator
        .delete_selected(coordinator.session().design_identity())
        .expect("whole computed Fillet deletion");
    let deleted_owner = coordinator
        .lineage_document()
        .step(owner)
        .expect("retained computed Fillet tombstone");
    assert_eq!(deleted_owner.state, LineageStepState::Tombstoned);
    assert_eq!(deleted_owner.outputs, initial_step.outputs);
    assert_eq!(
        deleted_owner.output_identities,
        initial_step.output_identities
    );
    assert_eq!(deleted_owner.reservations, initial_step.reservations);
    assert!(
        LineageMaterializationMap::derive(coordinator.lineage_document())
            .expect("deleted computed owner map")
            .live_owned_outputs(owner)
            .is_empty()
    );
    let deleted_cold = RetainedEditorCoordinator::lineage_materialization_checkpoint(
        &coordinator
            .lineage_session_json()
            .expect("deleted computed Fillet lineage"),
    )
    .expect("cold deleted computed Fillet");
    assert!(
        ComputedFeatureDocument::from_json(deleted_cold.feature_json())
            .expect("cold computed-feature document")
            .feature(feature)
            .is_none()
    );

    coordinator.undo().expect("Undo computed Fillet deletion");
    let ComputedFeatureDefinition::FilletSet(restored) = &coordinator
        .feature_document()
        .feature(feature)
        .expect("restored computed Fillet")
        .definition;
    assert_eq!(restored.radius.to_bits(), 0.75_f64.to_bits());
    assert_eq!(restored.corners[0].id, corner);
    assert_eq!(
        coordinator
            .lineage_document()
            .step(owner)
            .expect("restored computed owner"),
        &after_radius
    );
    coordinator.redo().expect("Redo computed Fillet deletion");
    assert_eq!(
        coordinator
            .lineage_document()
            .step(owner)
            .expect("redeleted computed owner")
            .state,
        LineageStepState::Tombstoned
    );
}
