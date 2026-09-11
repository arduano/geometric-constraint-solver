// SPDX-License-Identifier: GPL-3.0-or-later
use geosolve_constraint_editor::{AuthoringOptions, EditorScene, SelectionItem, Viewport};
use geosolve_sketch::{CurveSpan, DocumentDimensionMode};
use geosolve_sketch_code::{CodeProject, CompiledManagedSource, ProjectKey};
use geosolve_sketch_engine::{
    EditableSession, ToolOperationCommand, ToolOperationEvent, ToolOperationSample,
    ToolOperationTool,
};
fn project() -> String {
    CodeProject::managed(
        ProjectKey("tool-operations".into()),
        CompiledManagedSource::from_json(include_str!("fixtures/authoring-radius-2.json")).unwrap(),
    )
    .unwrap()
    .to_canonical_json()
    .unwrap()
}
fn viewport() -> Viewport {
    Viewport::new([800.0, 600.0], [0.0, 0.0], 5.0).unwrap()
}
fn command(session: &EditableSession, tool: ToolOperationTool) -> ToolOperationCommand {
    let circle = session
        .accepted()
        .result()
        .geometry
        .curves
        .iter()
        .find(|curve| {
            matches!(
                curve.curve.definition,
                geosolve_sketch::CurveDefinition::Circle { .. }
            )
        })
        .unwrap();
    let operand = session
        .tool_operation_operand(
            SelectionItem::Curve(CurveSpan {
                curve: circle.curve.id,
                segment: 0,
            }),
            Some(0.0),
        )
        .unwrap();
    if tool == ToolOperationTool::ToggleGeometryRole {
        let prediction = session
            .begin_tool_operation(tool, 41, viewport(), vec![operand])
            .unwrap();
        assert!(prediction.frame().unwrap().completed);
        return prediction.finish(41).unwrap().command().clone();
    }
    let mut prediction = session
        .begin_tool_operation(tool, 41, viewport(), Vec::new())
        .unwrap();
    if matches!(
        tool,
        ToolOperationTool::Radius | ToolOperationTool::Diameter
    ) {
        prediction
            .advance(
                41,
                ToolOperationSample {
                    sequence: 1,
                    input: ToolOperationEvent::AuthoringOptions {
                        options: AuthoringOptions {
                            dimension_mode: DocumentDimensionMode::Reference,
                            ..AuthoringOptions::default()
                        },
                    },
                },
            )
            .unwrap();
        let frame = prediction
            .advance(
                41,
                ToolOperationSample {
                    sequence: 2,
                    input: ToolOperationEvent::Pick { operand },
                },
            )
            .unwrap();
        assert!(frame.completed, "{:?}", frame.diagnostic);
    } else {
        panic!("unsupported test tool");
    }
    prediction.finish(41).unwrap().command().clone()
}
#[test]
fn all_existing_native_operation_tools_enter_isolated_collectors() {
    let session = EditableSession::open(&project(), None).unwrap();
    let before = session.state();
    for tool in ToolOperationTool::ALL {
        let prediction = session
            .begin_tool_operation(tool, 41, viewport(), Vec::new())
            .unwrap();
        let frame = prediction.frame().unwrap();
        assert!(!frame.completed, "{tool:?}");
        assert!(!frame.can_finish, "{tool:?}");
        EditorScene::from_detached_json(&prediction.scene_json().unwrap()).unwrap();
        prediction.cancel();
        assert_eq!(session.state(), before);
    }
}
#[test]
fn circular_dimensions_use_exact_semantic_operands_and_native_replay() {
    let session = EditableSession::open(&project(), None).unwrap();
    let before = session.state();
    for tool in [ToolOperationTool::Radius, ToolOperationTool::Diameter] {
        let command = command(&session, tool);
        let prepared = session.prepare_tool_operation(&command).unwrap();
        assert_eq!(prepared.declarations().len(), 1);
        assert!(
            command.expected_declarations[0]
                .builder_path
                .first()
                .is_some_and(|v| v == "dimension")
        );
        let mut forged = command.clone();
        forged.expected_declarations[0].symbol.push_str("forged");
        assert!(session.prepare_tool_operation(&forged).is_err());
        let mut stale = command;
        stale.basis.push_str("stale");
        assert!(session.prepare_tool_operation(&stale).is_err());
        assert_eq!(session.state(), before);
    }
}
#[test]
fn invalid_tool_inputs_preserve_exact_draft_and_accepted_origin() {
    let session = EditableSession::open(&project(), None).unwrap();
    let before = session.state();
    let mut prediction = session
        .begin_tool_operation(ToolOperationTool::Coincident, 41, viewport(), Vec::new())
        .unwrap();
    let frame = prediction.frame().unwrap();
    for (gesture, sequence, input) in [
        (99, 1, ToolOperationEvent::Complete),
        (41, 2, ToolOperationEvent::Complete),
        (
            41,
            1,
            ToolOperationEvent::Move {
                position: [f64::NAN, 0.0],
            },
        ),
    ] {
        assert!(
            prediction
                .advance(gesture, ToolOperationSample { sequence, input })
                .is_err()
        );
        assert_eq!(prediction.frame().unwrap(), frame);
        assert_eq!(session.state(), before);
    }
}
#[test]
#[ignore = "writes genuine compiler requests for fixture regeneration"]
fn write_real_compiler_requests() {
    let session = EditableSession::open(&project(), None).unwrap();
    let mut requests = serde_json::Map::new();
    for (name, tool) in [
        ("radius", ToolOperationTool::Radius),
        ("diameter", ToolOperationTool::Diameter),
        ("role", ToolOperationTool::ToggleGeometryRole),
    ] {
        let prepared = session
            .prepare_tool_operation(&command(&session, tool))
            .unwrap();
        requests.insert(
            name.into(),
            serde_json::to_value(prepared.request()).unwrap(),
        );
    }
    for tool in ToolOperationTool::ALL {
        let session = EditableSession::open(&family_project_for(tool), None).unwrap();
        let prepared = session
            .prepare_tool_operation(&family_command(&session, tool))
            .unwrap();
        requests.insert(
            format!("family-{tool:?}"),
            serde_json::to_value(prepared.request()).unwrap(),
        );
    }
    std::fs::create_dir_all("../../target/m98").unwrap();
    std::fs::write(
        "../../target/m98/tool-operation-requests.json",
        serde_json::to_string_pretty(&requests).unwrap(),
    )
    .unwrap();
}
fn family_project_for(tool: ToolOperationTool) -> String {
    CodeProject::managed(
        ProjectKey("tool-families".into()),
        CompiledManagedSource::from_json(
            if matches!(tool, ToolOperationTool::Fillet | ToolOperationTool::Offset) {
                include_str!("fixtures/tool-operation-feature.json")
            } else {
                include_str!("fixtures/tool-operation-basis.json")
            },
        )
        .unwrap(),
    )
    .unwrap()
    .to_canonical_json()
    .unwrap()
}
fn family_operands(
    session: &EditableSession,
    tool: ToolOperationTool,
) -> Vec<geosolve_sketch_engine::ToolOperationOperand> {
    let geometry = |label: &str, curve: bool| {
        let result = session.accepted().result();
        let key = result
            .named_outputs
            .iter()
            .find(|(key, output)| {
                output.reference.declaration.0 == label
                    && (!curve || !result.named_geometry[*key].spans.is_empty())
            })
            .map(|(key, _)| key)
            .unwrap();
        &result.named_geometry[key]
    };
    let curve = |label: &str, parameter: f64| {
        let span = geometry(label, true).spans[0];
        session
            .tool_operation_operand(SelectionItem::Curve(span), Some(parameter))
            .unwrap()
    };
    let point = |label: &str| {
        let point = geometry(label, false).points[0];
        session
            .tool_operation_operand(SelectionItem::Point(point), None)
            .unwrap()
    };
    match tool {
        ToolOperationTool::Lock => vec![point("left")],
        ToolOperationTool::Coincident => vec![point("left"), point("coincident")],
        ToolOperationTool::Horizontal | ToolOperationTool::SegmentLength => {
            vec![curve("hline", 0.5)]
        }
        ToolOperationTool::Vertical => vec![curve("vline", 0.5)],
        ToolOperationTool::Concentric => vec![curve("circle", 0.0), curve("concentric", 0.0)],
        ToolOperationTool::Collinear => vec![curve("hline", 0.5), curve("collinear", 0.5)],
        ToolOperationTool::Parallel | ToolOperationTool::Equal => {
            vec![curve("hline", 0.5), curve("parallel", 0.5)]
        }
        ToolOperationTool::Perpendicular | ToolOperationTool::OrientedAngle => {
            vec![curve("hline", 0.5), curve("vline", 0.5)]
        }
        ToolOperationTool::Midpoint => vec![point("midpoint"), curve("hline", 0.5)],
        ToolOperationTool::Symmetric => vec![point("left"), point("right"), curve("axis", 0.5)],
        ToolOperationTool::Tangent => vec![
            curve("tangent", 0.5),
            curve("circle", std::f64::consts::FRAC_PI_2),
        ],
        ToolOperationTool::Continuity => vec![curve("incoming", 1.0), curve("outgoing", 0.0)],
        ToolOperationTool::PointDistance => vec![point("left"), point("right")],
        ToolOperationTool::Radius
        | ToolOperationTool::Diameter
        | ToolOperationTool::ToggleGeometryRole => vec![curve("circle", 0.0)],
        ToolOperationTool::Offset => vec![curve("corner", 0.5)],
        ToolOperationTool::Fillet => {
            let value = session
                .accepted()
                .result()
                .geometry
                .curves
                .iter()
                .find(|value| value.curve.id == geometry("corner", true).spans[0].curve)
                .unwrap();
            let geosolve_sketch::CurveDefinition::Polyline { points, .. } = &value.curve.definition
            else {
                panic!("not polyline")
            };
            vec![
                session
                    .tool_operation_operand(SelectionItem::Point(points[1]), None)
                    .unwrap(),
            ]
        }
    }
}
fn family_command(session: &EditableSession, tool: ToolOperationTool) -> ToolOperationCommand {
    let operands = family_operands(session, tool);
    let mut prediction = session
        .begin_tool_operation(tool, 93, viewport(), Vec::new())
        .unwrap();
    let mut sequence = 0;
    let mut event = |prediction: &mut geosolve_sketch_engine::ToolOperationPrediction, input| {
        sequence += 1;
        let frame = prediction
            .advance(93, ToolOperationSample { sequence, input })
            .unwrap();
        assert!(
            frame.diagnostic.is_none(),
            "{tool:?}: {:?}",
            frame.diagnostic
        );
        frame
    };
    if matches!(
        tool,
        ToolOperationTool::PointDistance
            | ToolOperationTool::SegmentLength
            | ToolOperationTool::Radius
            | ToolOperationTool::Diameter
            | ToolOperationTool::OrientedAngle
    ) {
        event(
            &mut prediction,
            ToolOperationEvent::AuthoringOptions {
                options: AuthoringOptions {
                    dimension_mode: DocumentDimensionMode::Reference,
                    ..AuthoringOptions::default()
                },
            },
        );
    }
    if tool == ToolOperationTool::Tangent {
        event(
            &mut prediction,
            ToolOperationEvent::AuthoringOptions {
                options: AuthoringOptions {
                    tangent_orientation: geosolve_sketch::TangentOrientation::Opposed,
                    ..AuthoringOptions::default()
                },
            },
        );
    }
    for operand in operands {
        event(&mut prediction, ToolOperationEvent::Pick { operand });
    }
    if tool == ToolOperationTool::Fillet {
        event(
            &mut prediction,
            ToolOperationEvent::FilletRadius { radius: 2.0 },
        );
    }
    if tool == ToolOperationTool::Offset {
        event(
            &mut prediction,
            ToolOperationEvent::OffsetDistance { distance: 2.0 },
        );
    }
    if !prediction.frame().unwrap().completed {
        event(&mut prediction, ToolOperationEvent::Complete);
    }
    assert!(prediction.frame().unwrap().completed, "{tool:?}");
    prediction.finish(93).unwrap().command().clone()
}
#[test]
fn all_existing_native_tool_families_prepare_real_source_mutations() {
    let mut failures = Vec::new();
    for tool in ToolOperationTool::ALL {
        let session = EditableSession::open(&family_project_for(tool), None).unwrap();
        let before = session.state();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let command = family_command(&session, tool);
            let prepared = session.prepare_tool_operation(&command).unwrap();
            assert!(prepared.declarations().len() > 0);
        }));
        if result.is_err() {
            failures.push(tool);
        }
        assert_eq!(session.state(), before);
    }
    assert!(failures.is_empty(), "failing tool families: {failures:?}");
}

fn compilation(
    prepared: &geosolve_sketch_engine::PreparedToolOperation,
    name: &str,
) -> geosolve_sketch_code::PreparedManagedMutationReceipt {
    let fixtures: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/tool-operation-compilations.json")).unwrap();
    let compiled: CompiledManagedSource = serde_json::from_value(fixtures[name].clone()).unwrap();
    serde_json::from_value(serde_json::json!({"ticketDigest":prepared.request().ticket.ticket_digest,"baseSourceDigest":prepared.request().current.ir.source_digest,"candidateSourceDigest":compiled.ir.source_digest,"compiled":compiled})).unwrap()
}
#[test]
fn every_existing_tool_compiler_receipt_matches_native_preview_cold_restore_and_history() {
    let mut failures = Vec::new();
    for tool in ToolOperationTool::ALL
        .into_iter()
        .chain([ToolOperationTool::ToggleGeometryRole])
    {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let source = if tool == ToolOperationTool::ToggleGeometryRole {
                project()
            } else {
                family_project_for(tool)
            };
            let client = EditableSession::open(&source, None).unwrap();
            let command = if tool == ToolOperationTool::ToggleGeometryRole {
                command(&client, tool)
            } else {
                family_command(&client, tool)
            };
            let mut server = EditableSession::open(&source, None).unwrap();
            let before = server.state();
            let prepared = server.prepare_tool_operation(&command).unwrap();
            let name = if tool == ToolOperationTool::ToggleGeometryRole {
                "role".into()
            } else {
                format!("family-{tool:?}")
            };
            let candidate = server
                .resolve_tool_operation(&prepared, compilation(&prepared, &name))
                .unwrap_or_else(|error| panic!("{tool:?}: {error}"));
            assert_eq!(server.state(), before);
            assert!(candidate.result().validation.hard_residuals_validated);
            assert!(candidate.result().validation.all_active_features_current);
            assert!(
                candidate
                    .result()
                    .validation
                    .maximum_normalized_hard_residual
                    .is_none_or(|value| value.is_finite() && value <= 1e-9)
            );
            assert!(
                candidate
                    .result()
                    .geometry
                    .points
                    .iter()
                    .all(|point| point.position.into_iter().all(f64::is_finite))
            );
            assert!(
                candidate
                    .result()
                    .geometry
                    .scalars
                    .iter()
                    .all(|scalar| scalar.value.is_finite())
            );
            let cold = EditableSession::open(
                candidate.project_json(),
                Some(&serde_json::to_string(candidate.design()).unwrap()),
            )
            .unwrap();
            assert_eq!(
                cold.source_design_digest().unwrap(),
                candidate.source_design_digest()
            );
            let expected = candidate.result().geometry.clone();
            server.apply_tool_operation_commit(candidate).unwrap();
            assert_eq!(server.accepted().result().geometry, expected);
            assert_eq!(server.token().revision, before.token.revision + 1);
            let token = server.token().clone();
            server.undo(&token).unwrap();
            assert_eq!(server.accepted().result().geometry, before.result.geometry);
            let token = server.token().clone();
            server.redo(&token).unwrap();
            assert_eq!(server.accepted().result().geometry, expected);
        }));
        if result.is_err() {
            failures.push(tool);
        }
    }
    assert!(
        failures.is_empty(),
        "compiler/native failures: {failures:?}"
    );
}
#[test]
fn independent_reference_dimensions_replay_after_peer_addition() {
    let basis = EditableSession::open(&project(), None).unwrap();
    let radius = command(&basis, ToolOperationTool::Radius);
    let diameter = command(&basis, ToolOperationTool::Diameter);
    let mut latest = EditableSession::open(&project(), None).unwrap();
    let prepared = latest.prepare_tool_operation(&radius).unwrap();
    let accepted = latest
        .resolve_tool_operation(&prepared, compilation(&prepared, "radius"))
        .unwrap();
    latest.apply_tool_operation_commit(accepted).unwrap();
    let (prepared, witness) = latest
        .prepare_tool_operation_replay(&basis, &diameter)
        .unwrap();
    assert_eq!(
        witness.required_stable_declarations,
        vec![geosolve_sketch_code::SemanticSymbol("bore".into())]
    );
    assert_eq!(prepared.declarations().len(), 1);
    assert_ne!(
        witness.allocation_mapping[0].provisional,
        witness.allocation_mapping[0].persistent
    );
}
#[test]
fn preselection_options_and_excess_tree_operands_preserve_native_intent() {
    let session = EditableSession::open(&project(), None).unwrap();
    let circle = session.accepted().result().geometry.curves[0].curve.id;
    let operand = session
        .tool_operation_operand(
            SelectionItem::Curve(CurveSpan {
                curve: circle,
                segment: 0,
            }),
            Some(0.0),
        )
        .unwrap();
    let options = geosolve_sketch_engine::ToolOperationOptions {
        authoring_options: Some(AuthoringOptions {
            dimension_mode: DocumentDimensionMode::Reference,
            ..AuthoringOptions::default()
        }),
        ..Default::default()
    };
    let prediction = session
        .begin_tool_operation_with_options(
            ToolOperationTool::Radius,
            66,
            viewport(),
            vec![operand.clone()],
            options,
        )
        .unwrap();
    assert!(prediction.frame().unwrap().completed);
    let command = prediction.finish(66).unwrap().command().clone();
    assert_eq!(
        command.options.authoring_options.unwrap().dimension_mode,
        DocumentDimensionMode::Reference
    );
    session.prepare_tool_operation(&command).unwrap();
    let mut prediction = session
        .begin_tool_operation(ToolOperationTool::Radius, 67, viewport(), Vec::new())
        .unwrap();
    let frame = prediction
        .advance(
            67,
            ToolOperationSample {
                sequence: 1,
                input: ToolOperationEvent::PickSelection {
                    operands: vec![operand.clone(), operand],
                },
            },
        )
        .unwrap();
    assert!(!frame.completed);
    assert!(frame.diagnostic.is_some());
}
#[test]
fn rejected_fillet_option_edit_retains_preview_and_accepts_the_next_sequence() {
    let session =
        EditableSession::open(&family_project_for(ToolOperationTool::Fillet), None).unwrap();
    let curve = &session.accepted().result().geometry.curves[0].curve;
    let geosolve_sketch::CurveDefinition::Polyline { points, .. } = &curve.definition else {
        panic!("polyline")
    };
    let operand = session
        .tool_operation_operand(SelectionItem::Point(points[1]), None)
        .unwrap();
    let mut prediction = session
        .begin_tool_operation(ToolOperationTool::Fillet, 68, viewport(), vec![operand])
        .unwrap();
    let frame = prediction.frame().unwrap();
    let scene = prediction.scene_json().unwrap();
    let rejected = prediction
        .advance(
            68,
            ToolOperationSample {
                sequence: 1,
                input: ToolOperationEvent::FilletRadius { radius: -1.0 },
            },
        )
        .unwrap();
    assert_eq!(rejected.sequence, 1);
    assert!(rejected.diagnostic.is_some());
    assert_eq!(rejected.fillet_options, frame.fillet_options);
    assert_eq!(prediction.scene_json().unwrap(), scene);
    let accepted = prediction
        .advance(
            68,
            ToolOperationSample {
                sequence: 2,
                input: ToolOperationEvent::FilletRadius { radius: 2.0 },
            },
        )
        .unwrap();
    assert!(accepted.diagnostic.is_none());
    assert!(accepted.can_finish);
    assert_eq!(accepted.fillet_corners.len(), 1);
}

#[test]
fn repeated_tool_selection_batches_cannot_exceed_the_reserved_trace_bytes() {
    let session = EditableSession::open(&project(), None).unwrap();
    let accepted = session.state();
    let mut prediction = session
        .begin_tool_operation(ToolOperationTool::Parallel, 72, viewport(), Vec::new())
        .unwrap();
    let operand = geosolve_sketch_engine::ToolOperationOperand::Datum {
        datum: geosolve_sketch::SketchDatum::XAxis,
    };
    let mut retained_bytes = 0;
    let mut rejected = false;
    for sequence in 1..=140 {
        let sample = ToolOperationSample {
            sequence,
            input: ToolOperationEvent::PickSelection {
                operands: vec![operand.clone(); 256],
            },
        };
        let sample_bytes = serde_json::to_vec(&sample).unwrap().len();
        let prior = prediction.frame().unwrap();
        match prediction.advance(72, sample) {
            Ok(_) => retained_bytes += sample_bytes,
            Err(error) => {
                assert!(error.to_string().contains("trace byte limit"), "{error}");
                assert!(retained_bytes <= 1024 * 1024);
                assert!(retained_bytes + sample_bytes > 1024 * 1024);
                assert_eq!(prediction.frame().unwrap(), prior);
                // A rejected large event neither consumes the sequence nor poisons the draft.
                prediction
                    .advance(
                        72,
                        ToolOperationSample {
                            sequence,
                            input: ToolOperationEvent::Reset,
                        },
                    )
                    .unwrap();
                rejected = true;
                break;
            }
        }
    }
    assert!(
        rejected,
        "accepted {retained_bytes} sample bytes beyond the 1 MiB reservation"
    );
    assert_eq!(session.state(), accepted);
}

#[test]
fn camera_changes_update_native_pick_tolerance_and_replay_the_initial_camera() {
    let session =
        EditableSession::open(&family_project_for(ToolOperationTool::Radius), None).unwrap();
    let accepted = session.state();
    let initial_camera = viewport();
    let zoomed_camera = Viewport::new([800.0, 600.0], [100.0, 100.0], 50.0).unwrap();
    let mut prediction = session
        .begin_tool_operation_with_options(
            ToolOperationTool::Radius,
            73,
            initial_camera,
            Vec::new(),
            geosolve_sketch_engine::ToolOperationOptions {
                authoring_options: Some(AuthoringOptions {
                    dimension_mode: DocumentDimensionMode::Reference,
                    ..AuthoringOptions::default()
                }),
                ..geosolve_sketch_engine::ToolOperationOptions::default()
            },
        )
        .unwrap();
    prediction
        .advance(
            73,
            ToolOperationSample {
                sequence: 1,
                input: ToolOperationEvent::Viewport {
                    viewport: zoomed_camera,
                },
            },
        )
        .unwrap();
    // Half a model unit is 25 pixels after zoom, outside the native pick tolerance.
    let missed = prediction
        .advance(
            73,
            ToolOperationSample {
                sequence: 2,
                input: ToolOperationEvent::Click {
                    position: [105.5, 100.0],
                },
            },
        )
        .unwrap();
    assert!(!missed.completed);
    assert!(!missed.has_pending);
    prediction
        .advance(
            73,
            ToolOperationSample {
                sequence: 3,
                input: ToolOperationEvent::Viewport {
                    viewport: initial_camera,
                },
            },
        )
        .unwrap();
    // The same point is only 2.5 pixels from the circle after zooming out.
    let picked = prediction
        .advance(
            73,
            ToolOperationSample {
                sequence: 4,
                input: ToolOperationEvent::Click {
                    position: [105.5, 100.0],
                },
            },
        )
        .unwrap();
    assert!(picked.completed, "{:?}", picked.diagnostic);
    let terminal = prediction.finish(73).unwrap();
    assert_eq!(terminal.command().viewport, initial_camera);
    assert_eq!(
        terminal.command().samples[0].input,
        ToolOperationEvent::Viewport {
            viewport: zoomed_camera
        }
    );
    session.prepare_tool_operation(terminal.command()).unwrap();
    assert_eq!(session.state(), accepted);
}

#[test]
fn reset_clears_native_operands_and_keeps_each_tool_ready_for_another_pick() {
    for tool in [
        ToolOperationTool::PointDistance,
        ToolOperationTool::Fillet,
        ToolOperationTool::Offset,
    ] {
        let session = EditableSession::open(&family_project_for(tool), None).unwrap();
        let accepted = session.state();
        let curve = &session.accepted().result().geometry.curves[0].curve;
        let pick = if tool == ToolOperationTool::PointDistance {
            ToolOperationEvent::Click {
                position: [10.0, 30.0],
            }
        } else {
            ToolOperationEvent::Pick {
                operand: session
                    .tool_operation_operand(
                        SelectionItem::Curve(CurveSpan {
                            curve: curve.id,
                            segment: 0,
                        }),
                        Some(0.5),
                    )
                    .unwrap(),
            }
        };
        let mut prediction = session
            .begin_tool_operation(tool, 74, viewport(), Vec::new())
            .unwrap();
        assert!(!prediction.frame().unwrap().has_pending, "{tool:?}");
        let pending = prediction
            .advance(
                74,
                ToolOperationSample {
                    sequence: 1,
                    input: pick.clone(),
                },
            )
            .unwrap();
        assert!(
            pending.has_pending && pending.can_reset && pending.can_step_back,
            "{tool:?}: {pending:?}"
        );
        let reset = prediction
            .advance(
                74,
                ToolOperationSample {
                    sequence: 2,
                    input: ToolOperationEvent::Reset,
                },
            )
            .unwrap();
        assert!(
            !reset.has_pending && !reset.can_reset && !reset.can_step_back,
            "{tool:?}: {reset:?}"
        );
        assert!(!reset.can_finish && !reset.completed, "{tool:?}");
        let repeated = prediction
            .advance(
                74,
                ToolOperationSample {
                    sequence: 3,
                    input: pick,
                },
            )
            .unwrap();
        assert!(repeated.has_pending, "{tool:?}: {repeated:?}");
        assert_eq!(session.state(), accepted);
    }
}

#[test]
fn empty_reset_and_step_back_preserve_the_active_tool_and_options() {
    for tool in [
        ToolOperationTool::Radius,
        ToolOperationTool::PointDistance,
        ToolOperationTool::Fillet,
        ToolOperationTool::Offset,
    ] {
        let session = EditableSession::open(&family_project_for(tool), None).unwrap();
        let accepted = session.state();
        let position = match tool {
            ToolOperationTool::Radius => [105.0, 100.0],
            ToolOperationTool::PointDistance => [10.0, 30.0],
            _ => [210.0, 0.0],
        };
        for clear in [ToolOperationEvent::Reset, ToolOperationEvent::StepBack] {
            let mut prediction = session
                .begin_tool_operation_with_options(
                    tool,
                    75,
                    viewport(),
                    Vec::new(),
                    geosolve_sketch_engine::ToolOperationOptions {
                        authoring_options: matches!(
                            tool,
                            ToolOperationTool::Radius | ToolOperationTool::PointDistance
                        )
                        .then_some(AuthoringOptions {
                            dimension_mode: DocumentDimensionMode::Reference,
                            ..AuthoringOptions::default()
                        }),
                        ..geosolve_sketch_engine::ToolOperationOptions::default()
                    },
                )
                .unwrap();
            let before = prediction.frame().unwrap();
            assert!(!before.has_pending);
            let reset = prediction
                .advance(
                    75,
                    ToolOperationSample {
                        sequence: 1,
                        input: clear,
                    },
                )
                .unwrap();
            assert!(!reset.has_pending);
            assert_eq!(reset.authoring_options, before.authoring_options);
            assert_eq!(reset.fillet_options, before.fillet_options);
            assert_eq!(reset.offset_distance, before.offset_distance);
            let picked = prediction
                .advance(
                    75,
                    ToolOperationSample {
                        sequence: 2,
                        input: ToolOperationEvent::Click { position },
                    },
                )
                .unwrap();
            assert!(
                picked.completed || picked.has_pending,
                "{tool:?}: {picked:?}"
            );
            assert_eq!(session.state(), accepted);
        }
    }
}

#[test]
fn refused_relation_clears_terminal_operands_and_allows_a_valid_retry() {
    let project = CodeProject::managed(
        ProjectKey("tool-rejection".into()),
        CompiledManagedSource::from_json(include_str!("fixtures/tool-operation-rejection.json"))
            .unwrap(),
    )
    .unwrap()
    .to_canonical_json()
    .unwrap();
    let session = EditableSession::open(&project, None).unwrap();
    let accepted = session.state();
    let mut prediction = session
        .begin_tool_operation(ToolOperationTool::Horizontal, 76, viewport(), Vec::new())
        .unwrap();
    let before = prediction.scene_json().unwrap();
    let rejected = prediction
        .advance(
            76,
            ToolOperationSample {
                sequence: 1,
                input: ToolOperationEvent::Click {
                    position: [10.0, 5.0],
                },
            },
        )
        .unwrap();
    assert!(!rejected.completed);
    assert!(rejected.diagnostic.is_some());
    assert!(
        rejected.pending.is_empty() && !rejected.has_pending,
        "{rejected:?}"
    );
    assert_eq!(prediction.scene_json().unwrap(), before);
    assert_eq!(session.state(), accepted);
    let retried = prediction
        .advance(
            76,
            ToolOperationSample {
                sequence: 2,
                input: ToolOperationEvent::Click {
                    position: [10.0, 30.0],
                },
            },
        )
        .unwrap();
    assert!(retried.completed, "{retried:?}");
    assert!(retried.diagnostic.is_none());
    let terminal = prediction.finish(76).unwrap();
    assert_eq!(terminal.command().expected_declarations.len(), 1);
    session.prepare_tool_operation(terminal.command()).unwrap();
    assert_eq!(session.state(), accepted);
}
