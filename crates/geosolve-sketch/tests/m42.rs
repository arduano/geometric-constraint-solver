use geosolve_core::{
    CancellationToken, OperationCheckpoint, OperationControl, OperationLimits, OperationOutcome,
    OperationStopReason, OperationWorkCounter, SolverConfig,
};
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DocumentCommand, DocumentDimensionDefinition,
    DocumentDimensionMode, DocumentEdit, DocumentElementId, DocumentParameterId,
    DocumentParameterKind, DocumentParameterTarget, DocumentScalarBranch,
    DocumentScalarPropertyRef, DocumentScalarUnit, DocumentSessionError, DocumentSolveRequest,
    MAX_PARAMETER_BATCH_ENTRIES, ParameterBatch, ParameterBatchEntry, ParameterValue, PersistentId,
    RetainedSketchDocumentSession, RuntimeSource, ScalarDomain, ScalarUnit, SketchDocument,
    SketchDocumentSession,
};

fn batch(
    revision: u64,
    parameter: geosolve_sketch::DocumentParameterId,
    value: f64,
) -> ParameterBatch {
    ParameterBatch::new(
        revision,
        vec![ParameterBatchEntry {
            parameter,
            value: ParameterValue::Length(value),
        }],
    )
    .unwrap()
}

fn parameterized_rectangle() -> (
    SketchDocument,
    geosolve_sketch::RectangleIds,
    DocumentParameterId,
    DocumentParameterId,
    geosolve_sketch::DocumentDimensionId,
) {
    let mut document = SketchDocument::new(6.0).unwrap();
    let rectangle = document
        .add_rectangle("rectangle", [0.0, 0.0], 4.0, 3.0)
        .unwrap();
    let input = document
        .add_parameter("width", DocumentParameterKind::Length)
        .unwrap();
    document
        .add_parameter_binding(
            input,
            DocumentParameterTarget::DrivingDimension(rectangle.dimensions[0]),
        )
        .unwrap();
    let target = document
        .add_scalar(
            "reference target",
            1.0,
            ScalarUnit::Length,
            ScalarDomain::Finite,
        )
        .unwrap();
    let reference = document
        .add_dimension(
            "reference width",
            DocumentDimensionDefinition::CurveLength {
                curve: CurveSpan::line(rectangle.curves[0]),
                target,
            },
            DocumentDimensionMode::Reference,
        )
        .unwrap();
    let output = document
        .add_parameter("reported width", DocumentParameterKind::Length)
        .unwrap();
    document.add_parameter_output(output, reference).unwrap();
    (document, rectangle, input, output, reference)
}

#[test]
fn batches_are_canonical_and_reject_invalid_entries() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let first = document
        .add_parameter("first", DocumentParameterKind::Length)
        .unwrap();
    let second = document
        .add_parameter("second", DocumentParameterKind::Length)
        .unwrap();
    let ordered = ParameterBatch::new(
        4,
        vec![
            ParameterBatchEntry {
                parameter: first,
                value: ParameterValue::Length(1.0),
            },
            ParameterBatchEntry {
                parameter: second,
                value: ParameterValue::Length(2.0),
            },
        ],
    )
    .unwrap();
    let reversed =
        ParameterBatch::new(4, ordered.entries().iter().rev().copied().collect()).unwrap();
    assert_eq!(ordered, reversed);
    assert_eq!(ordered.digest(), reversed.digest());
    assert!(
        ParameterBatch::new(
            5,
            vec![
                ParameterBatchEntry {
                    parameter: first,
                    value: ParameterValue::Length(1.0)
                },
                ParameterBatchEntry {
                    parameter: first,
                    value: ParameterValue::Length(2.0)
                },
            ],
        )
        .is_err()
    );
    assert!(
        ParameterBatch::new(
            5,
            vec![ParameterBatchEntry {
                parameter: first,
                value: ParameterValue::Length(f64::NAN)
            }],
        )
        .is_err()
    );
}

#[test]
fn shared_input_drives_dimensions_and_reference_output_is_stamped() {
    let mut document = SketchDocument::new(6.0).unwrap();
    let rectangle = document
        .add_rectangle("rectangle", [0.0, 0.0], 4.0, 3.0)
        .unwrap();
    let input = document
        .add_parameter("size", DocumentParameterKind::Length)
        .unwrap();
    let output = document
        .add_parameter("measured size", DocumentParameterKind::Length)
        .unwrap();
    for dimension in rectangle.dimensions {
        document
            .add_parameter_binding(input, DocumentParameterTarget::DrivingDimension(dimension))
            .unwrap();
    }
    let target = document
        .add_scalar(
            "reference target",
            1.0,
            ScalarUnit::Length,
            ScalarDomain::Finite,
        )
        .unwrap();
    let reference = document
        .add_dimension(
            "reference width",
            DocumentDimensionDefinition::CurveLength {
                curve: CurveSpan::line(rectangle.curves[0]),
                target,
            },
            DocumentDimensionMode::Reference,
        )
        .unwrap();
    document.add_parameter_output(output, reference).unwrap();

    let host_batch = batch(7, input, 5.0);
    let session = RetainedSketchDocumentSession::new_with_parameter_batch(
        document,
        host_batch.clone(),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let accepted = session
        .accepted_state()
        .expect("parameterized solve is accepted");
    assert_eq!(accepted.input().parameter_revision(), 7);
    assert_eq!(accepted.input().parameter_digest(), host_batch.digest());
    assert_eq!(accepted.mappings().parameter_bindings().len(), 2);
    let proposals = accepted.parameter_output_proposals();
    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].parameter, output);
    assert_eq!(proposals[0].parameter_revision, 7);
    assert_eq!(proposals[0].parameter_digest, host_batch.digest());
    assert!((proposals[0].value - 5.0).abs() < 1.0e-7);
}

#[test]
fn stale_parameter_batch_is_rejected_without_changing_accepted_state() {
    let mut document = SketchDocument::new(6.0).unwrap();
    let rectangle = document
        .add_rectangle("rectangle", [0.0, 0.0], 4.0, 3.0)
        .unwrap();
    let input = document
        .add_parameter("width", DocumentParameterKind::Length)
        .unwrap();
    document
        .add_parameter_binding(
            input,
            DocumentParameterTarget::DrivingDimension(rectangle.dimensions[0]),
        )
        .unwrap();

    let retained = batch(7, input, 5.0);
    let mut session = RetainedSketchDocumentSession::new_with_parameter_batch(
        document,
        retained.clone(),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let accepted = session.accepted_state().unwrap().identity();
    let design = session.design_identity();

    let error = session
        .update_parameter_batch(
            design,
            batch(6, input, 4.0),
            DocumentSolveRequest::default(),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        DocumentSessionError::StaleParameterRevision {
            actual: 6,
            retained: 7
        }
    ));
    assert_eq!(session.parameter_batch(), &retained);
    assert_eq!(session.accepted_state().unwrap().identity(), accepted);
}

#[test]
fn controlled_parameterized_lowering_exhaustion_does_not_publish() {
    let mut document = SketchDocument::new(6.0).unwrap();
    let rectangle = document
        .add_rectangle("rectangle", [0.0, 0.0], 4.0, 3.0)
        .unwrap();
    let input = document
        .add_parameter("width", DocumentParameterKind::Length)
        .unwrap();
    document
        .add_parameter_binding(
            input,
            DocumentParameterTarget::DrivingDimension(rectangle.dimensions[0]),
        )
        .unwrap();
    let point = rectangle.points[0];
    let mut session = RetainedSketchDocumentSession::new_with_parameter_batch(
        document,
        batch(7, input, 5.0),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let design = session.design_identity();
    let accepted = session.accepted_state().unwrap().identity();
    let mut limits = OperationLimits::unlimited();
    limits.document_lowering_items = 0;

    let outcome = session
        .apply_controlled(
            design,
            DocumentEdit::SetPointPosition {
                point,
                position: [0.5, 0.5],
            },
            OperationControl::new(CancellationToken::default(), limits),
        )
        .unwrap();
    let OperationOutcome::WorkExhausted { report } = outcome else {
        panic!("zero lowering allowance must stop the parameterized retained edit");
    };
    assert_eq!(report.consumed.document_lowering_items, 0);
    assert_eq!(
        report.stopping_reason,
        Some(OperationStopReason::WorkExhausted {
            counter: OperationWorkCounter::DocumentLoweringItems,
            checkpoint: OperationCheckpoint::DocumentLowering,
        })
    );
    assert_eq!(session.design_identity(), design);
    assert_eq!(session.accepted_state().unwrap().identity(), accepted);
    assert_eq!(session.parameter_batch().revision(), 7);
}

#[test]
fn one_angle_input_drives_compatible_dimensions_without_a_parameter_unknown() {
    let mut document = SketchDocument::new(4.0).unwrap();
    let origin = document.add_point("origin", [0.0, 0.0]).unwrap();
    let x_end = document.add_point("x end", [2.0, 0.0]).unwrap();
    let y_end = document.add_point("y end", [0.0, 2.0]).unwrap();
    let x_line = document
        .add_curve(
            "x line",
            CurveDefinition::Line {
                start: origin,
                end: x_end,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let y_line = document
        .add_curve(
            "y line",
            CurveDefinition::Line {
                start: origin,
                end: y_end,
                branch_direction: [0.0, 1.0],
            },
        )
        .unwrap();
    let angle = document
        .add_parameter("angle", DocumentParameterKind::Angle)
        .unwrap();
    let first_target = document
        .add_scalar(
            "first angle",
            std::f64::consts::FRAC_PI_2,
            ScalarUnit::Angle,
            ScalarDomain::Positive,
        )
        .unwrap();
    let second_target = document
        .add_scalar(
            "second angle",
            std::f64::consts::FRAC_PI_2,
            ScalarUnit::Angle,
            ScalarDomain::Positive,
        )
        .unwrap();
    let definition = |target| DocumentDimensionDefinition::OrientedAngle {
        first: CurveSpan::line(x_line),
        second: CurveSpan::line(y_line),
        target,
        orientation: geosolve_sketch::DocumentAngleOrientation::CounterClockwise,
    };
    let first = document
        .add_dimension(
            "first angle",
            definition(first_target),
            DocumentDimensionMode::Driving,
        )
        .unwrap();
    let second = document
        .add_dimension(
            "second angle",
            definition(second_target),
            DocumentDimensionMode::Driving,
        )
        .unwrap();
    for dimension in [first, second] {
        document
            .add_parameter_binding(angle, DocumentParameterTarget::DrivingDimension(dimension))
            .unwrap();
    }

    let batch = ParameterBatch::new(
        1,
        vec![ParameterBatchEntry {
            parameter: angle,
            value: ParameterValue::Angle(std::f64::consts::FRAC_PI_2),
        }],
    )
    .unwrap();
    let session = RetainedSketchDocumentSession::new_with_parameter_batch(
        document,
        batch.clone(),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let accepted = session.accepted_state().unwrap();
    assert_eq!(accepted.input().parameter_revision(), 1);
    assert_eq!(accepted.input().parameter_digest(), batch.digest());
    assert_eq!(accepted.mappings().parameter_bindings().len(), 2);
    assert!(
        accepted
            .mappings()
            .parameter_bindings()
            .iter()
            .all(|binding| binding.parameter == angle)
    );
}

#[test]
fn parameter_activation_overlay_does_not_clear_user_suppression() {
    let mut document = SketchDocument::new(4.0).unwrap();
    let rectangle = document
        .add_rectangle("rectangle", [0.0, 0.0], 2.0, 2.0)
        .unwrap();
    let activation = document
        .add_parameter("active", DocumentParameterKind::Activation)
        .unwrap();
    document
        .add_parameter_binding(
            activation,
            DocumentParameterTarget::Activation(DocumentElementId::Curve(rectangle.curves[0])),
        )
        .unwrap();
    document
        .set_element_user_suppressed(DocumentElementId::Point(rectangle.points[0]), true)
        .unwrap();
    let mut session = RetainedSketchDocumentSession::new_with_parameter_batch(
        document,
        ParameterBatch::new(
            1,
            vec![ParameterBatchEntry {
                parameter: activation,
                value: ParameterValue::Activation(false),
            }],
        )
        .unwrap(),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    assert!(
        session
            .accepted_state()
            .unwrap()
            .mappings()
            .runtime_curve(rectangle.curves[0])
            .is_none()
    );

    let design = session.design_identity();
    session
        .update_parameter_batch(
            design,
            ParameterBatch::new(
                2,
                vec![ParameterBatchEntry {
                    parameter: activation,
                    value: ParameterValue::Activation(true),
                }],
            )
            .unwrap(),
            DocumentSolveRequest::default(),
        )
        .unwrap();
    let accepted = session.accepted_state().unwrap();
    assert_eq!(accepted.input().parameter_revision(), 2);
    assert!(
        accepted
            .document()
            .effective_activity()
            .reason(rectangle.points[0])
            .is_some()
    );
    assert!(
        accepted
            .mappings()
            .runtime_curve(rectangle.curves[0])
            .is_none()
    );
}

#[test]
fn invalid_batches_retain_prior_accepted_geometry_input_and_proposals() {
    let (document, _, input, _, _) = parameterized_rectangle();
    let retained = batch(1, input, 4.0);
    let mut session = RetainedSketchDocumentSession::new_with_parameter_batch(
        document,
        retained.clone(),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let accepted = session.accepted_state().unwrap();
    let geometry = accepted.solve_result().geometry.clone();
    let input_stamp = accepted.input();
    let proposals = accepted.parameter_output_proposals().to_vec();
    let design = session.design_identity();
    let unknown = DocumentParameterId(PersistentId::from_u128(u128::MAX));
    let invalid = [
        ParameterBatch::new(2, Vec::new()).unwrap(),
        ParameterBatch::new(
            3,
            vec![ParameterBatchEntry {
                parameter: unknown,
                value: ParameterValue::Length(4.0),
            }],
        )
        .unwrap(),
        ParameterBatch::new(
            4,
            vec![ParameterBatchEntry {
                parameter: input,
                value: ParameterValue::Angle(1.0),
            }],
        )
        .unwrap(),
        batch(5, input, -1.0),
    ];
    for candidate in invalid {
        session
            .update_parameter_batch(design, candidate, DocumentSolveRequest::default())
            .unwrap();
        let accepted = session.accepted_state().unwrap();
        assert_eq!(accepted.solve_result().geometry, geometry);
        assert_eq!(accepted.input(), input_stamp);
        assert_eq!(accepted.parameter_output_proposals(), proposals);
        assert!(session.last_attempt().failure().is_some());
    }
    assert!(
        ParameterBatch::new(
            5,
            vec![ParameterBatchEntry {
                parameter: input,
                value: ParameterValue::Length(f64::INFINITY),
            }],
        )
        .is_err()
    );
    assert!(
        ParameterBatch::new(
            5,
            vec![
                ParameterBatchEntry {
                    parameter: input,
                    value: ParameterValue::Length(1.0),
                };
                MAX_PARAMETER_BATCH_ENTRIES + 1
            ],
        )
        .is_err()
    );
}

#[test]
fn input_output_ownership_overlap_rejects_atomically() {
    let (mut document, rectangle, input, _, reference) = parameterized_rectangle();
    let before = document.clone();
    assert!(document.add_parameter_output(input, reference).is_err());
    assert_eq!(document, before);

    let output = document
        .add_parameter("other output", DocumentParameterKind::Length)
        .unwrap();
    document.add_parameter_output(output, reference).unwrap();
    let before = document.clone();
    assert!(
        document
            .add_parameter_binding(
                output,
                DocumentParameterTarget::DrivingDimension(rectangle.dimensions[1]),
            )
            .is_err()
    );
    assert_eq!(document, before);
}

#[test]
fn v4_export_is_frozen_while_draft_v5_round_trips_m42_state() {
    let document = SketchDocument::new(2.0).unwrap();
    let v4 = document.to_canonical_json().unwrap();
    assert_eq!(document.to_canonical_json().unwrap(), v4);

    let (document, _, _, _, _) = parameterized_rectangle();
    assert!(document.to_canonical_json().is_err());
    let draft = document.to_draft_v5_json().unwrap();
    let restored = SketchDocument::from_draft_v5_json(&draft).unwrap();
    assert_eq!(restored.to_draft_v5_json().unwrap(), draft);
    assert_eq!(restored.parameters(), document.parameters());
    assert_eq!(restored.parameter_bindings(), document.parameter_bindings());
    assert_eq!(restored.parameter_outputs(), document.parameter_outputs());

    let mut document = SketchDocument::new(3.0).unwrap();
    let (_, property) = add_parameterized_ellipse(&mut document, "ellipse", 0.0);
    let parameter = document
        .add_parameter("ratio", DocumentParameterKind::Dimensionless)
        .unwrap();
    document
        .add_parameter_binding(
            parameter,
            DocumentParameterTarget::DimensionlessFixedScalar(property),
        )
        .unwrap();
    let draft = document.to_draft_v5_json().unwrap();
    let restored = SketchDocument::from_draft_v5_json(&draft).unwrap();
    assert_eq!(restored.parameter_bindings(), document.parameter_bindings());
    assert_eq!(restored.to_draft_v5_json().unwrap(), draft);
}

#[test]
fn parameter_declarations_bindings_and_outputs_are_undoable_commands() {
    let mut document = SketchDocument::new(4.0).unwrap();
    let rectangle = document
        .add_rectangle("rectangle", [0.0, 0.0], 2.0, 1.0)
        .unwrap();
    let target = document
        .add_scalar(
            "reference target",
            1.0,
            ScalarUnit::Length,
            ScalarDomain::Finite,
        )
        .unwrap();
    let reference = document
        .add_dimension(
            "reference",
            DocumentDimensionDefinition::CurveLength {
                curve: CurveSpan::line(rectangle.curves[0]),
                target,
            },
            DocumentDimensionMode::Reference,
        )
        .unwrap();
    let mut session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let parameter = match session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::CreateParameter {
                label: "input".into(),
                kind: DocumentParameterKind::Length,
            },
        ))
        .unwrap()
        .effect
        .unwrap()
    {
        geosolve_sketch::DocumentCommandEffect::CreatedParameter(parameter) => parameter,
        other => panic!("unexpected effect: {other:?}"),
    };
    for edit in [
        DocumentEdit::AddParameterBinding {
            parameter,
            target: DocumentParameterTarget::DrivingDimension(rectangle.dimensions[1]),
        },
        DocumentEdit::RemoveParameterBinding {
            parameter,
            target: DocumentParameterTarget::DrivingDimension(rectangle.dimensions[1]),
        },
        DocumentEdit::AddParameterOutput {
            parameter,
            dimension: reference,
        },
        DocumentEdit::RemoveParameterOutput {
            parameter,
            dimension: reference,
        },
    ] {
        session
            .apply(DocumentCommand::new(session.revision(), edit))
            .unwrap();
    }
    assert_eq!(session.history_len(), 5);
    assert!(session.document().parameter_bindings().is_empty());
    assert!(session.document().parameter_outputs().is_empty());
    for _ in 0..5 {
        session.undo(session.revision()).unwrap();
    }
    assert!(session.document().parameters().is_empty());
    for _ in 0..5 {
        session.redo(session.revision()).unwrap();
    }
    assert_eq!(session.document().parameters().len(), 1);
    assert!(session.document().parameter_bindings().is_empty());
    assert!(session.document().parameter_outputs().is_empty());
}

#[test]
fn rejected_parameter_attempt_publishes_no_replacement_proposals() {
    let (document, _, input, _, _) = parameterized_rectangle();
    let mut session = RetainedSketchDocumentSession::new_with_parameter_batch(
        document,
        batch(1, input, 4.0),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let accepted = session.accepted_state().unwrap();
    let identity = accepted.identity();
    let proposals = accepted.parameter_output_proposals().to_vec();
    session
        .update_parameter_batch(
            session.design_identity(),
            ParameterBatch::new(2, Vec::new()).unwrap(),
            DocumentSolveRequest::default(),
        )
        .unwrap();
    assert!(session.last_attempt().failure().is_some());
    let accepted = session.accepted_state().unwrap();
    assert_eq!(accepted.identity(), identity);
    assert_eq!(accepted.parameter_output_proposals(), proposals);
    assert_eq!(
        accepted.parameter_output_proposals()[0].accepted,
        accepted.identity()
    );
}

#[test]
fn canonical_batches_reproduce_public_accepted_evidence_exactly() {
    let (first_document, _, input, _, _) = parameterized_rectangle();
    let draft = first_document.to_draft_v5_json().unwrap();
    let second_document = SketchDocument::from_draft_v5_json(&draft).unwrap();
    let ordered = batch(9, input, 5.0);
    let canonical =
        ParameterBatch::new(9, ordered.entries().iter().rev().copied().collect()).unwrap();
    let first = RetainedSketchDocumentSession::new_with_parameter_batch(
        first_document,
        ordered,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let second = RetainedSketchDocumentSession::new_with_parameter_batch(
        second_document,
        canonical,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let first = first.accepted_state().unwrap();
    let second = second.accepted_state().unwrap();
    assert_eq!(first.input(), second.input());
    assert_eq!(first.solve_result(), second.solve_result());
    assert_eq!(
        first.parameter_output_proposals(),
        second.parameter_output_proposals()
    );
    assert_eq!(
        first.document().to_draft_v5_json().unwrap(),
        second.document().to_draft_v5_json().unwrap()
    );
}

#[test]
fn dimensionless_parameters_cannot_bind_current_driving_dimensions() {
    let mut document = SketchDocument::new(2.0).unwrap();
    let rectangle = document
        .add_rectangle("rectangle", [0.0, 0.0], 1.0, 1.0)
        .unwrap();
    let parameter = document
        .add_parameter("unitless", DocumentParameterKind::Dimensionless)
        .unwrap();
    let before = document.clone();
    assert!(
        document
            .add_parameter_binding(
                parameter,
                DocumentParameterTarget::DrivingDimension(rectangle.dimensions[0]),
            )
            .is_err()
    );
    assert_eq!(document, before);
}

fn add_parameterized_ellipse(
    document: &mut SketchDocument,
    label: &str,
    x: f64,
) -> (geosolve_sketch::CurveId, DocumentScalarPropertyRef) {
    let center = document
        .add_point(format!("{label} center"), [x, 0.0])
        .unwrap();
    let axis = document
        .add_point(format!("{label} axis"), [x + 2.0, 0.0])
        .unwrap();
    let domain = ScalarDomain::Bounded {
        lower: f64::from_bits(1),
        upper: 1.0,
    };
    let ratio = document
        .add_scalar(format!("{label} ratio"), 0.5, ScalarUnit::Parameter, domain)
        .unwrap();
    let curve = document
        .add_curve(
            label,
            CurveDefinition::Ellipse {
                center,
                major_axis_point: axis,
                minor_axis_ratio: ratio,
            },
        )
        .unwrap();
    (
        curve,
        DocumentScalarPropertyRef {
            scalar: ratio,
            unit: DocumentScalarUnit::Dimensionless,
            domain,
            branch: DocumentScalarBranch::Dimensionless,
        },
    )
}

fn assert_dimensionless_binding_audit(
    accepted: &geosolve_sketch::SketchAcceptedDocumentState,
    parameter: DocumentParameterId,
    value: f64,
    batch: &ParameterBatch,
) {
    assert_eq!(accepted.mappings().parameter_bindings().len(), 2);
    for binding in accepted.mappings().parameter_bindings() {
        assert_eq!(binding.parameter, parameter);
        assert_eq!(binding.value.to_bits(), value.to_bits());
        assert_eq!(binding.parameter_revision, batch.revision());
        assert_eq!(binding.parameter_digest, batch.digest());
        let RuntimeSource::Constraint(runtime) = binding.runtime else {
            panic!("dimensionless targets must lower through fixed-scalar constraints");
        };
        assert!(matches!(
            binding.target,
            DocumentParameterTarget::DimensionlessFixedScalar(_)
        ));
        let source = accepted
            .solve_result()
            .source_mappings
            .iter()
            .find(|mapping| mapping.source == geosolve_sketch::SketchSource::Constraint(runtime))
            .unwrap();
        assert_eq!(source.residual_ids.len(), 1);
        let audit = accepted
            .solve_result()
            .display_audit
            .sources
            .iter()
            .find(|audit| Some(audit.source_id) == source.core_source_id)
            .unwrap();
        assert_eq!(audit.rows.len(), 1);
        assert_eq!(audit.rows[0].template, "property - target");
        assert_eq!(audit.rows[0].incident_variables.len(), 1);
    }
}

fn assert_out_of_domain_dimensionless_batch_is_atomic(
    session: &mut RetainedSketchDocumentSession,
    parameter: DocumentParameterId,
) {
    let retained = session.accepted_state().unwrap();
    let identity = retained.identity();
    let input = retained.input();
    session
        .update_parameter_batch(
            session.design_identity(),
            ParameterBatch::new(
                14,
                vec![ParameterBatchEntry {
                    parameter,
                    value: ParameterValue::Dimensionless(1.25),
                }],
            )
            .unwrap(),
            DocumentSolveRequest::default(),
        )
        .unwrap();
    let retained = session.accepted_state().unwrap();
    assert_eq!(retained.identity(), identity);
    assert_eq!(retained.input(), input);
    assert!(matches!(
        session
            .last_attempt()
            .failure()
            .map(geosolve_sketch::SketchAttemptFailure::kind),
        Some(geosolve_sketch::SketchAttemptFailureKind::ParameterInput)
    ));
}

#[test]
fn one_dimensionless_input_executes_multiple_typed_fixed_scalar_targets_with_provenance() {
    let mut document = SketchDocument::new(8.0).unwrap();
    let (_, first) = add_parameterized_ellipse(&mut document, "first", 0.0);
    let (_, second) = add_parameterized_ellipse(&mut document, "second", 4.0);
    let parameter = document
        .add_parameter("ratio", DocumentParameterKind::Dimensionless)
        .unwrap();
    for property in [first, second] {
        document
            .add_parameter_binding(
                parameter,
                DocumentParameterTarget::DimensionlessFixedScalar(property),
            )
            .unwrap();
    }
    let batch = ParameterBatch::new(
        12,
        vec![ParameterBatchEntry {
            parameter,
            value: ParameterValue::Dimensionless(0.75),
        }],
    )
    .unwrap();

    let mut session = RetainedSketchDocumentSession::new_with_parameter_batch(
        document,
        batch.clone(),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let accepted = session.accepted_state().unwrap();
    assert!(accepted.solve_result().accepted());
    assert_dimensionless_binding_audit(accepted, parameter, 0.75, &batch);
    assert!((accepted.document().scalar(first.scalar).unwrap().value - 0.75).abs() < 1.0e-9);
    assert!((accepted.document().scalar(second.scalar).unwrap().value - 0.75).abs() < 1.0e-9);
    assert_eq!(
        session
            .design_document()
            .scalar(first.scalar)
            .unwrap()
            .value
            .to_bits(),
        0.5_f64.to_bits()
    );
    assert_eq!(
        session
            .design_document()
            .scalar(second.scalar)
            .unwrap()
            .value
            .to_bits(),
        0.5_f64.to_bits()
    );

    let accepted_before = accepted.identity();
    let replacement = ParameterBatch::new(
        13,
        vec![ParameterBatchEntry {
            parameter,
            value: ParameterValue::Dimensionless(0.625),
        }],
    )
    .unwrap();
    session
        .update_parameter_batch(
            session.design_identity(),
            replacement.clone(),
            DocumentSolveRequest::default(),
        )
        .unwrap();
    let accepted = session.accepted_state().unwrap();
    assert_ne!(accepted.identity(), accepted_before);
    assert_eq!(accepted.input().parameter_revision(), 13);
    assert_eq!(accepted.input().parameter_digest(), replacement.digest());
    assert!((accepted.document().scalar(first.scalar).unwrap().value - 0.625).abs() < 1.0e-9);
    assert!((accepted.document().scalar(second.scalar).unwrap().value - 0.625).abs() < 1.0e-9);
    assert_eq!(
        session
            .design_document()
            .scalar(first.scalar)
            .unwrap()
            .value
            .to_bits(),
        0.5_f64.to_bits()
    );

    assert_out_of_domain_dimensionless_batch_is_atomic(&mut session, parameter);
}

#[test]
fn activation_is_resolved_before_dimensionless_numeric_inputs_are_required() {
    let mut document = SketchDocument::new(4.0).unwrap();
    let (ellipse, property) = add_parameterized_ellipse(&mut document, "ellipse", 0.0);
    let activation = document
        .add_parameter("ellipse active", DocumentParameterKind::Activation)
        .unwrap();
    let ratio = document
        .add_parameter("ratio", DocumentParameterKind::Dimensionless)
        .unwrap();
    document
        .add_parameter_binding(
            activation,
            DocumentParameterTarget::Activation(DocumentElementId::Curve(ellipse)),
        )
        .unwrap();
    document
        .add_parameter_binding(
            ratio,
            DocumentParameterTarget::DimensionlessFixedScalar(property),
        )
        .unwrap();

    let inactive = ParameterBatch::new(
        1,
        vec![ParameterBatchEntry {
            parameter: activation,
            value: ParameterValue::Activation(false),
        }],
    )
    .unwrap();
    let session = RetainedSketchDocumentSession::new_with_parameter_batch(
        document.clone(),
        inactive,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    assert!(
        session
            .accepted_state()
            .unwrap()
            .mappings()
            .parameter_bindings()
            .is_empty()
    );

    let active_without_ratio = ParameterBatch::new(
        2,
        vec![ParameterBatchEntry {
            parameter: activation,
            value: ParameterValue::Activation(true),
        }],
    )
    .unwrap();
    let rejected = RetainedSketchDocumentSession::new_with_parameter_batch(
        document,
        active_without_ratio,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    assert!(rejected.accepted_state().is_none());
    assert!(matches!(
        rejected
            .last_attempt()
            .failure()
            .map(geosolve_sketch::SketchAttemptFailure::kind),
        Some(geosolve_sketch::SketchAttemptFailureKind::ParameterInput)
    ));
}

#[test]
fn malformed_or_non_executable_dimensionless_targets_are_rejected_atomically() {
    let mut document = SketchDocument::new(2.0).unwrap();
    let scalar = document
        .add_scalar(
            "generic parameter",
            0.5,
            ScalarUnit::Parameter,
            ScalarDomain::Finite,
        )
        .unwrap();
    let parameter = document
        .add_parameter("input", DocumentParameterKind::Dimensionless)
        .unwrap();
    let before = document.clone();
    assert!(
        document
            .add_parameter_binding(
                parameter,
                DocumentParameterTarget::DimensionlessFixedScalar(DocumentScalarPropertyRef {
                    scalar,
                    unit: DocumentScalarUnit::Dimensionless,
                    domain: ScalarDomain::Finite,
                    branch: DocumentScalarBranch::Dimensionless,
                }),
            )
            .is_err()
    );
    assert_eq!(document, before);

    let (_, property) = add_parameterized_ellipse(&mut document, "ellipse", 1.0);
    for malformed in [
        DocumentScalarPropertyRef {
            unit: DocumentScalarUnit::Parameter,
            ..property
        },
        DocumentScalarPropertyRef {
            branch: DocumentScalarBranch::Unsigned,
            ..property
        },
    ] {
        let before = document.clone();
        assert!(
            document
                .add_parameter_binding(
                    parameter,
                    DocumentParameterTarget::DimensionlessFixedScalar(malformed),
                )
                .is_err()
        );
        assert_eq!(document, before);
    }

    document
        .add_parameter_binding(
            parameter,
            DocumentParameterTarget::DimensionlessFixedScalar(property),
        )
        .unwrap();
    let second_supplier = document
        .add_parameter("second supplier", DocumentParameterKind::Dimensionless)
        .unwrap();
    let before = document.clone();
    assert!(
        document
            .add_parameter_binding(
                second_supplier,
                DocumentParameterTarget::DimensionlessFixedScalar(property),
            )
            .is_err()
    );
    assert_eq!(document, before);
}
