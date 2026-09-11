// SPDX-License-Identifier: GPL-3.0-or-later
use geosolve_constraint_editor::{EditorScene, GeometryToolVariant, Viewport};
use geosolve_sketch::GeometryRole;
use geosolve_sketch_code::{CodeProject, CompiledManagedSource, ProjectKey};
use geosolve_sketch_engine::{
    ConstructionCommand, ConstructionEvent, ConstructionSample, ConstructionTool, EditableSession,
};

fn project() -> String {
    CodeProject::managed(
        ProjectKey("construction-parity".into()),
        CompiledManagedSource::from_json(include_str!(
            "../../../packages/geosolve-sketch-code/test/fixtures/managed-clean-segment.json"
        ))
        .unwrap(),
    )
    .unwrap()
    .to_canonical_json()
    .unwrap()
}
fn viewport() -> Viewport {
    Viewport::new([800.0, 600.0], [0.0, 0.0], 20.0).unwrap()
}
fn samples(tool: ConstructionTool) -> Vec<ConstructionEvent> {
    use ConstructionTool as T;
    let points = match tool {
        T::SketchPoint => vec![[10.0, 10.0]],
        T::Segment
        | T::MidpointLine
        | T::TwoPointAlignedRectangle
        | T::CenterRectangle
        | T::CenterRadiusCircle
        | T::TwoPointDiameterCircle
        | T::Parabola
        | T::Hyperbola => vec![[10.0, 10.0], [14.0, 12.0]],
        T::Polyline
        | T::ThreePointCornerRectangle
        | T::ThreePointCenterRectangle
        | T::ThreePointCircle
        | T::CenterArc
        | T::QuadraticBezier
        | T::RationalQuadraticConic => vec![[10.0, 10.0], [14.0, 10.0], [12.0, 13.0]],
        T::ThreePointArc => vec![[14.0, 10.0], [10.0, 10.0], [12.0, 12.0]],
        T::TangentArc => vec![[2.0, 0.0], [3.0, 1.0]],
        T::CenterAxesEllipse => vec![[10.0, 10.0], [14.0, 10.0], [10.0, 12.0]],
        T::AxisEndpointsEllipse => vec![[14.0, 10.0], [6.0, 10.0], [10.0, 12.0]],
        T::CenterAxesEllipticalArc => vec![
            [10.0, 10.0],
            [14.0, 10.0],
            [10.0, 12.0],
            [14.0, 10.0],
            [10.0, 12.0],
        ],
        T::AxisEndpointsEllipticalArc => vec![
            [14.0, 10.0],
            [6.0, 10.0],
            [10.0, 12.0],
            [14.0, 10.0],
            [10.0, 12.0],
        ],
        T::CubicBezier | T::OpenControlNurbs | T::PeriodicControlNurbs => {
            vec![[10.0, 10.0], [14.0, 10.0], [14.0, 14.0], [10.0, 14.0]]
        }
    };
    let mut events = Vec::new();
    for position in points {
        for click in [false, true] {
            events.push(if click {
                ConstructionEvent::Click {
                    position,
                    suppressed: true,
                    regularized: false,
                }
            } else {
                ConstructionEvent::Move {
                    position,
                    suppressed: true,
                    regularized: false,
                }
            });
        }
    }
    if matches!(
        tool,
        T::Polyline | T::OpenControlNurbs | T::PeriodicControlNurbs
    ) {
        events.push(ConstructionEvent::Complete);
    }
    events
}
fn command(session: &EditableSession, tool: ConstructionTool) -> ConstructionCommand {
    let before = session.state();
    let mut prediction = session
        .begin_construction(tool, 71, viewport(), GeometryRole::Profile)
        .unwrap();
    let mut completed = false;
    for (i, input) in samples(tool).into_iter().enumerate() {
        let frame = prediction
            .advance(
                71,
                ConstructionSample {
                    sequence: u32::try_from(i + 1).unwrap(),
                    input,
                },
            )
            .unwrap_or_else(|e| panic!("{tool:?} sample {i}: {e}"));
        assert!(
            frame.diagnostic.is_none(),
            "{tool:?} sample {i}: {:?}",
            frame.diagnostic
        );
        assert_eq!(session.state(), before);
        if let Some(guide) = frame.preview {
            let wire = serde_json::to_string(&guide).unwrap();
            assert_eq!(
                serde_json::from_str::<geosolve_sketch_engine::ConstructionGuide>(&wire).unwrap(),
                guide
            );
        }
        completed = frame.completed;
    }
    assert!(completed, "{tool:?} has no accepted terminal");
    let scene = EditorScene::from_detached_json(&prediction.scene_json().unwrap()).unwrap();
    let document = scene.presentation_document();
    assert!(
        document
            .points()
            .iter()
            .flat_map(|p| p.position)
            .all(f64::is_finite)
    );
    assert!(document.scalars().iter().all(|p| p.value.is_finite()));
    assert!(document.points().iter().any(|p| p.position == [0.0, 0.0]));
    assert!(
        document
            .points()
            .iter()
            .any(|p| p.position.map(f64::to_bits) == [2.0_f64, 0.0].map(f64::to_bits))
    );
    prediction
        .finish(71)
        .unwrap_or_else(|e| panic!("{tool:?} source projection: {e}"))
        .command()
        .clone()
}

#[test]
fn all_native_construction_recipes_retain_exact_identity_and_replay_source_meaning() {
    assert_eq!(
        ConstructionTool::ALL.map(ConstructionTool::variant),
        GeometryToolVariant::ALL
    );
    for tool in ConstructionTool::ALL {
        let key = serde_json::to_value(tool).unwrap();
        assert_eq!(key, tool.variant().key().replace('-', "_"));
        let client = EditableSession::open(&project(), None).unwrap();
        let intent = command(&client, tool);
        assert!(!intent.expected_declarations.is_empty(), "{tool:?}");
        let wire = serde_json::to_string(&intent).unwrap();
        let decoded: ConstructionCommand = serde_json::from_str(&wire).unwrap();
        assert_eq!(intent, decoded);
        let server = EditableSession::open(&project(), None).unwrap();
        let before = server.state();
        let prepared = server
            .prepare_construction(&decoded)
            .unwrap_or_else(|e| panic!("{tool:?} server replay: {e}"));
        assert_eq!(
            prepared.declarations().count(),
            intent.expected_declarations.len()
        );
        assert_eq!(server.state(), before);
        let mut forged = decoded;
        forged.expected_declarations[0].symbol = "forged-recipe".into();
        assert!(server.prepare_construction(&forged).is_err(), "{tool:?}");
        assert_eq!(server.state(), before);
    }
}

#[test]
#[ignore = "explicit real compiler request generation"]
fn write_all_recipe_compiler_requests() {
    let mut requests = serde_json::Map::new();
    for tool in ConstructionTool::ALL {
        let session = EditableSession::open(&project(), None).unwrap();
        let command = command(&session, tool);
        let prepared = session.prepare_construction(&command).unwrap();
        requests.insert(
            tool.variant().key().into(),
            serde_json::to_value(prepared.request()).unwrap(),
        );
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/m98/construction-parity-requests.json");
    std::fs::write(root, serde_json::to_string_pretty(&requests).unwrap()).unwrap();
}

#[test]
fn all_recipes_validate_real_compiler_results_cold_reconstruction_and_atomic_history() {
    let fixtures: std::collections::BTreeMap<String, CompiledManagedSource> = serde_json::from_str(
        include_str!("fixtures/construction-parity-compilations.json"),
    )
    .unwrap();
    assert_eq!(fixtures.len(), ConstructionTool::ALL.len());
    let mut failures = Vec::new();
    for tool in ConstructionTool::ALL {
        let mut server = EditableSession::open(&project(), None).unwrap();
        let before = server.state();
        let command = command(&server, tool);
        let prepared = server.prepare_construction(&command).unwrap();
        let compiled = &fixtures[tool.variant().key()];
        let receipt = serde_json::from_value(serde_json::json!({
            "ticketDigest": prepared.request().ticket.ticket_digest,
            "baseSourceDigest": prepared.request().current.ir.source_digest,
            "candidateSourceDigest": compiled.ir.source_digest,
            "compiled": compiled,
        }))
        .unwrap();
        let staged = match server.resolve_construction(&prepared, receipt) {
            Ok(staged) => staged,
            Err(error) => {
                failures.push(format!("{tool:?}: {error}"));
                continue;
            }
        };
        assert_eq!(server.state(), before);
        assert!(staged.result().validation.hard_residuals_validated);
        assert!(staged.result().validation.all_active_features_current);
        assert!(
            staged
                .result()
                .geometry
                .points
                .iter()
                .flat_map(|p| p.position)
                .all(f64::is_finite)
        );
        assert!(
            staged
                .result()
                .geometry
                .scalars
                .iter()
                .all(|p| p.value.is_finite())
        );
        assert!(staged.result().geometry.points.len() > before.result.geometry.points.len());
        let cold = EditableSession::open(
            staged.project_json(),
            Some(&serde_json::to_string(staged.design()).unwrap()),
        )
        .unwrap();
        assert_eq!(
            cold.source_design_digest().unwrap(),
            staged.source_design_digest()
        );
        assert_eq!(
            source_owned_geometry(&cold.accepted().result().geometry),
            source_owned_geometry(&staged.result().geometry),
            "{tool:?}: cold authored geometry"
        );
        server.apply_construction_commit(staged).unwrap();
        assert_eq!(server.token().revision, before.token.revision + 1);
        let created = server.state();
        let token = server.token().clone();
        server.undo(&token).unwrap();
        assert_eq!(server.accepted().result().geometry, before.result.geometry);
        let token = server.token().clone();
        server.redo(&token).unwrap();
        assert_eq!(server.accepted().result().geometry, created.result.geometry);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one ordered native option trace retains branch, rejection and compiler replay evidence"
)]
fn native_options_and_explicit_branch_are_ordered_replay_intent_and_bad_options_are_retryable() {
    use geosolve_constraint_editor::{ConicConstructionOptions, NurbsConstructionOptions};
    use geosolve_sketch::DocumentArcSweep;
    let session = EditableSession::open(&project(), None).unwrap();
    let before = session.state();
    for (tool, option) in [
        (
            ConstructionTool::RationalQuadraticConic,
            ConstructionEvent::ConicOptions {
                options: ConicConstructionOptions {
                    middle_weight: 0.5,
                    ..ConicConstructionOptions::default()
                },
            },
        ),
        (
            ConstructionTool::OpenControlNurbs,
            ConstructionEvent::NurbsOptions {
                options: NurbsConstructionOptions {
                    degree: 2,
                    weights: vec![1.0, 2.0, 1.0, 0.5],
                    ..NurbsConstructionOptions::default()
                },
            },
        ),
        (
            ConstructionTool::CenterArc,
            ConstructionEvent::ConicOptions {
                options: ConicConstructionOptions {
                    arc_sweep: DocumentArcSweep::Clockwise,
                    ..ConicConstructionOptions::default()
                },
            },
        ),
    ] {
        let mut prediction = session
            .begin_construction(tool, 71, viewport(), GeometryRole::Construction)
            .unwrap();
        let invalid = ConstructionSample {
            sequence: 1,
            input: ConstructionEvent::NurbsOptions {
                options: NurbsConstructionOptions {
                    degree: 0,
                    ..NurbsConstructionOptions::default()
                },
            },
        };
        assert!(prediction.advance(71, invalid).is_err());
        prediction
            .advance(
                71,
                ConstructionSample {
                    sequence: 1,
                    input: option.clone(),
                },
            )
            .unwrap();
        for (i, input) in samples(tool).into_iter().enumerate() {
            let frame = prediction
                .advance(
                    71,
                    ConstructionSample {
                        sequence: u32::try_from(i + 2).unwrap(),
                        input,
                    },
                )
                .unwrap();
            assert!(
                frame.diagnostic.is_none(),
                "{tool:?}: {:?}",
                frame.diagnostic
            );
        }
        let command = prediction.finish(71).unwrap().command().clone();
        assert_eq!(command.samples[0].input, option);
        let native_wire = serde_json::to_string(&command).unwrap();
        let decoded: ConstructionCommand = serde_json::from_str(&native_wire).unwrap();
        assert_eq!(command, decoded);
        session.prepare_construction(&decoded).unwrap();
        let mut forged = decoded;
        forged.samples[0].input = match tool {
            ConstructionTool::OpenControlNurbs => ConstructionEvent::NurbsOptions {
                options: NurbsConstructionOptions::default(),
            },
            _ => ConstructionEvent::ConicOptions {
                options: ConicConstructionOptions::default(),
            },
        };
        assert!(
            session.prepare_construction(&forged).is_err(),
            "{tool:?}: changed authored options must fail witness authentication"
        );
        assert_eq!(session.state(), before);
    }

    let mut prediction = session
        .begin_construction(
            ConstructionTool::CenterArc,
            71,
            viewport(),
            GeometryRole::Profile,
        )
        .unwrap();
    let click = |sequence, position| ConstructionSample {
        sequence,
        input: ConstructionEvent::Click {
            position,
            suppressed: true,
            regularized: false,
        },
    };
    prediction.advance(71, click(1, [10.0, 10.0])).unwrap();
    prediction.advance(71, click(2, [14.0, 10.0])).unwrap();
    prediction
        .advance(
            71,
            ConstructionSample {
                sequence: 3,
                input: ConstructionEvent::FlipBranch,
            },
        )
        .unwrap();
    let frame = prediction
        .advance(
            71,
            ConstructionSample {
                sequence: 4,
                input: ConstructionEvent::Move {
                    position: [10.0, 14.0],
                    suppressed: true,
                    regularized: false,
                },
            },
        )
        .unwrap();
    let Some(geosolve_sketch_engine::ConstructionGuide::CircularArc {
        sweep,
        sweep_radians,
        ..
    }) = frame.preview
    else {
        panic!("missing arc preview")
    };
    assert_eq!(sweep, DocumentArcSweep::Clockwise);
    assert!((sweep_radians - 1.5 * std::f64::consts::PI).abs() < 1e-9);
    prediction.advance(71, click(5, [10.0, 14.0])).unwrap();
    let command = prediction.finish(71).unwrap().command().clone();
    session.prepare_construction(&command).unwrap();
    assert_eq!(session.state(), before);
}

#[test]
fn every_construction_recipe_can_cancel_without_publishing_source_or_history() {
    let session = EditableSession::open(&project(), None).unwrap();
    let before = session.state();
    for tool in ConstructionTool::ALL {
        let mut prediction = session
            .begin_construction(tool, 71, viewport(), GeometryRole::Profile)
            .unwrap();
        prediction
            .advance(
                71,
                ConstructionSample {
                    sequence: 1,
                    input: samples(tool)[0].clone(),
                },
            )
            .unwrap();
        prediction.cancel();
        assert_eq!(session.state(), before, "{tool:?}");
    }
}

// A cold session intentionally owns a new numeric native namespace. Normalize
// only exact native IDs to their source-owned labels; retain all geometry,
// topology, roles, spans and explicit branch fields.
fn source_owned_geometry(geometry: &geosolve_sketch_engine::EngineGeometry) -> serde_json::Value {
    fn normalize(value: &mut serde_json::Value, ids: &std::collections::BTreeMap<String, String>) {
        match value {
            serde_json::Value::String(id) => {
                if let Some(label) = ids.get(id) {
                    *id = label.clone();
                }
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    normalize(value, ids);
                }
            }
            serde_json::Value::Object(values) => {
                for value in values.values_mut() {
                    normalize(value, ids);
                }
            }
            _ => {}
        }
    }
    let ids = geometry
        .points
        .iter()
        .map(|p| (serde_json::to_value(p.id).unwrap(), p.label.clone()))
        .chain(
            geometry
                .scalars
                .iter()
                .map(|p| (serde_json::to_value(p.id).unwrap(), p.label.clone())),
        )
        .chain(geometry.curves.iter().map(|p| {
            (
                serde_json::to_value(p.curve.id).unwrap(),
                p.curve.label.clone(),
            )
        }))
        .map(|(id, label)| {
            (
                id.as_str().expect("native ID wire string").to_owned(),
                label,
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut value = serde_json::to_value(geometry).unwrap();
    normalize(&mut value, &ids);
    for key in ["points", "scalars", "curves"] {
        value[key].as_array_mut().unwrap().sort_by_key(|entry| {
            if key == "curves" {
                entry["curve"]["label"].as_str().unwrap().to_owned()
            } else {
                entry["label"].as_str().unwrap().to_owned()
            }
        });
    }
    value
}

#[test]
fn snap_cycle_replays_native_candidate_choice_without_transporting_native_candidate_ids() {
    let session = EditableSession::open(&project(), None).unwrap();
    let mut prediction = session
        .begin_construction(
            ConstructionTool::Segment,
            71,
            viewport(),
            GeometryRole::Profile,
        )
        .unwrap();
    let at_origin = ConstructionEvent::Move {
        position: [0.0, 0.0],
        suppressed: false,
        regularized: false,
    };
    let first = prediction
        .advance(
            71,
            ConstructionSample {
                sequence: 1,
                input: at_origin,
            },
        )
        .unwrap();
    assert!(
        first.can_cycle_inference,
        "authored endpoint and intrinsic origin must offer native snap alternatives"
    );
    let cycled = prediction
        .advance(
            71,
            ConstructionSample {
                sequence: 2,
                input: ConstructionEvent::CycleInference,
            },
        )
        .unwrap();
    assert!(cycled.can_cycle_inference);
    assert_eq!(cycled.adjusted_position, Some([0.0, 0.0]));
    prediction
        .advance(
            71,
            ConstructionSample {
                sequence: 3,
                input: ConstructionEvent::Click {
                    position: [0.0, 0.0],
                    suppressed: false,
                    regularized: false,
                },
            },
        )
        .unwrap();
    let terminal = prediction
        .advance(
            71,
            ConstructionSample {
                sequence: 4,
                input: ConstructionEvent::Click {
                    position: [10.0, 3.0],
                    suppressed: true,
                    regularized: false,
                },
            },
        )
        .unwrap();
    assert!(terminal.completed);
    let command = prediction.finish(71).unwrap().command().clone();
    let wire = serde_json::to_string(&command).unwrap();
    assert!(!wire.contains("preferred_candidate"));
    session.prepare_construction(&command).unwrap();
}

#[test]
fn repeated_nurbs_option_arrays_respect_total_retained_trace_bytes() {
    let session = EditableSession::open(&project(), None).unwrap();
    let before = session.state();
    let mut prediction = session
        .begin_construction(
            ConstructionTool::OpenControlNurbs,
            71,
            viewport(),
            GeometryRole::Profile,
        )
        .unwrap();
    let options = geosolve_constraint_editor::NurbsConstructionOptions {
        weights: vec![1.0; 4096],
        ..Default::default()
    };
    let mut rejected = None;
    for sequence in 1..=256 {
        if let Err(error) = prediction.advance(
            71,
            ConstructionSample {
                sequence,
                input: ConstructionEvent::NurbsOptions {
                    options: options.clone(),
                },
            },
        ) {
            rejected = Some((sequence, error));
            break;
        }
    }
    let (sequence, error) =
        rejected.expect("total weight arrays must hit the one MiB trace reservation");
    assert!(error.to_string().contains("byte limit"));
    assert_eq!(prediction.frame().sequence, sequence - 1);
    assert_eq!(session.state(), before);
    prediction.cancel();
}

#[test]
fn circle_source_keeps_corrected_native_triplet_and_tangent_arc_keeps_opposed_branch() {
    let session = EditableSession::open(&project(), None).unwrap();
    let mut circle = session
        .begin_construction(
            ConstructionTool::ThreePointCircle,
            71,
            viewport(),
            GeometryRole::Profile,
        )
        .unwrap();
    let input = |sequence, position| ConstructionSample {
        sequence,
        input: ConstructionEvent::Click {
            position,
            suppressed: true,
            regularized: false,
        },
    };
    circle.advance(71, input(1, [10.0, 10.0])).unwrap();
    circle.advance(71, input(2, [14.0, 10.0])).unwrap();
    let invalid = circle.advance(71, input(3, [12.0, 10.0])).unwrap();
    assert!(!invalid.completed);
    assert!(invalid.diagnostic.is_some());
    circle
        .advance(
            71,
            ConstructionSample {
                sequence: 4,
                input: ConstructionEvent::StepBack,
            },
        )
        .unwrap();
    circle.advance(71, input(5, [15.0, 10.0])).unwrap();
    assert!(
        circle
            .advance(71, input(6, [12.0, 13.0]))
            .unwrap()
            .completed
    );
    let command = circle.finish(71).unwrap().command().clone();
    let declaration = command
        .expected_declarations
        .iter()
        .find(|declaration| declaration.builder_path == ["geometry", "threePointCircle"])
        .unwrap();
    let geosolve_sketch_code::ManagedValue::Object(arguments) = &declaration.arguments else {
        panic!("source arguments")
    };
    for (name, point) in [
        ("first", [10.0, 10.0]),
        ("second", [15.0, 10.0]),
        ("third", [12.0, 13.0]),
    ] {
        assert_eq!(
            arguments[name],
            geosolve_sketch_code::ManagedValue::Array(
                point
                    .into_iter()
                    .map(geosolve_sketch_code::ManagedValue::Number)
                    .collect()
            )
        );
    }
    session.prepare_construction(&command).unwrap();
    let mut arc = session
        .begin_construction(
            ConstructionTool::TangentArc,
            71,
            viewport(),
            GeometryRole::Profile,
        )
        .unwrap();
    arc.advance(71, input(1, [0.0, 0.0])).unwrap();
    assert!(arc.advance(71, input(2, [-1.0, 1.0])).unwrap().completed);
    let command = arc.finish(71).unwrap().command().clone();
    let declaration = command
        .expected_declarations
        .iter()
        .find(|declaration| declaration.builder_path == ["geometry", "tangentArc"])
        .unwrap();
    let geosolve_sketch_code::ManagedValue::Object(arguments) = &declaration.arguments else {
        panic!("source arguments")
    };
    assert_eq!(
        arguments["orientation"],
        geosolve_sketch_code::ManagedValue::String("opposed".into())
    );
    assert!(arguments.contains_key("center"));
    session.prepare_construction(&command).unwrap();
}

#[test]
fn construction_initial_frame_exposes_native_defaults_without_a_pointer_event() {
    let session = EditableSession::open(&project(), None).unwrap();
    let prediction = session
        .begin_construction(
            ConstructionTool::OpenControlNurbs,
            71,
            viewport(),
            GeometryRole::Profile,
        )
        .unwrap();
    let frame = prediction.frame();
    assert_eq!(frame.sequence, 0);
    assert!(!frame.completed);
    assert!(!frame.can_finish);
    assert_eq!(
        frame.nurbs_options,
        geosolve_constraint_editor::NurbsConstructionOptions::default()
    );
    assert_eq!(
        frame.conic_options,
        geosolve_constraint_editor::ConicConstructionOptions::default()
    );
    assert_eq!(frame.stage.as_deref(), Some("Control point"));
    assert_eq!(
        serde_json::from_str::<geosolve_sketch_engine::ConstructionFrame>(
            &serde_json::to_string(&frame).unwrap()
        )
        .unwrap(),
        frame
    );
}

#[test]
fn camera_changes_update_native_snap_tolerance_and_preserve_replay_origin() {
    let session = EditableSession::open(&project(), None).unwrap();
    let before = session.state();
    let mut prediction = session
        .begin_construction(
            ConstructionTool::Segment,
            71,
            viewport(),
            GeometryRole::Profile,
        )
        .unwrap();
    let first = prediction
        .advance(
            71,
            ConstructionSample {
                sequence: 1,
                input: ConstructionEvent::Move {
                    position: [0.1, 0.1],
                    suppressed: false,
                    regularized: false,
                },
            },
        )
        .unwrap();
    assert_eq!(first.adjusted_position, Some([0.0, 0.0]));
    let zoomed = Viewport::new([1024.0, 768.0], [1.0, 0.5], 200.0).unwrap();
    let changed = prediction
        .advance(
            71,
            ConstructionSample {
                sequence: 2,
                input: ConstructionEvent::Viewport { viewport: zoomed },
            },
        )
        .unwrap();
    assert_eq!(
        changed.adjusted_position, None,
        "no candidate lies within the zoomed pixel tolerance"
    );
    assert!(changed.inference_guides.is_empty());
    let invalid = Viewport {
        pixels_per_model_unit: 0.0,
        ..zoomed
    };
    assert!(
        prediction
            .advance(
                71,
                ConstructionSample {
                    sequence: 3,
                    input: ConstructionEvent::Viewport { viewport: invalid },
                }
            )
            .is_err()
    );
    assert_eq!(prediction.frame(), changed);
    for (sequence, position) in [(3, [0.1, 0.1]), (4, [10.0, 11.0])] {
        prediction
            .advance(
                71,
                ConstructionSample {
                    sequence,
                    input: ConstructionEvent::Click {
                        position,
                        suppressed: false,
                        regularized: false,
                    },
                },
            )
            .unwrap();
    }
    let scene = EditorScene::from_detached_json(&prediction.scene_json().unwrap()).unwrap();
    assert!(
        scene.presentation_document().points().iter().any(|point| {
            point
                .position
                .into_iter()
                .all(|value| (value - 0.1).abs() < 1e-12)
        }),
        "the first new endpoint must retain its unsnapped model position"
    );
    let terminal = prediction.finish(71).unwrap();
    assert_eq!(terminal.command().viewport, viewport());
    assert_eq!(
        terminal.command().samples[1].input,
        ConstructionEvent::Viewport { viewport: zoomed }
    );
    session.prepare_construction(terminal.command()).unwrap();
    assert_eq!(session.state(), before);
}

#[test]
fn reset_clears_native_stages_but_retains_recipe_options_and_repeatable_replay() {
    let session = EditableSession::open(&project(), None).unwrap();
    let before = session.state();
    for tool in ConstructionTool::ALL {
        let mut prediction = session
            .begin_construction(tool, 71, viewport(), GeometryRole::Profile)
            .unwrap();
        let initial = prediction.frame();
        assert!(!initial.has_pending && !initial.can_reset && !initial.can_step_back);
        let mut sequence = 0;
        if tool != ConstructionTool::SketchPoint {
            let first_click = samples(tool)
                .into_iter()
                .find(|input| matches!(input, ConstructionEvent::Click { .. }))
                .unwrap();
            sequence += 1;
            let pending = prediction
                .advance(
                    71,
                    ConstructionSample {
                        sequence,
                        input: first_click,
                    },
                )
                .unwrap();
            assert!(
                pending.has_pending && pending.can_reset && pending.can_step_back,
                "{tool:?}"
            );
        }
        for _ in 0..2 {
            sequence += 1;
            let reset = prediction
                .advance(
                    71,
                    ConstructionSample {
                        sequence,
                        input: ConstructionEvent::Reset,
                    },
                )
                .unwrap();
            assert!(
                !reset.has_pending && !reset.can_reset && !reset.can_step_back,
                "{tool:?}"
            );
            assert_eq!(reset.preview, None, "{tool:?}");
            assert_eq!(reset.conic_options, initial.conic_options);
            assert_eq!(reset.nurbs_options, initial.nurbs_options);
            assert_eq!(reset.stage, initial.stage);
        }
        sequence += 1;
        let after_camera = prediction
            .advance(
                71,
                ConstructionSample {
                    sequence,
                    input: ConstructionEvent::Viewport {
                        viewport: viewport(),
                    },
                },
            )
            .unwrap();
        assert_eq!(
            after_camera.preview, None,
            "old pointer must not reappear after Reset for {tool:?}"
        );
        for input in samples(tool) {
            sequence += 1;
            prediction
                .advance(71, ConstructionSample { sequence, input })
                .unwrap();
        }
        assert!(prediction.frame().completed, "{tool:?}");
        let terminal = prediction.finish(71).unwrap();
        session.prepare_construction(terminal.command()).unwrap();
        assert_eq!(session.state(), before);
    }
}
