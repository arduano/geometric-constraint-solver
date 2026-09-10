// SPDX-License-Identifier: GPL-3.0-or-later
use geosolve_constraint_editor::{EditorScene, Viewport};
use geosolve_sketch::GeometryRole;
use geosolve_sketch_code::{
    CodeProject, CompiledManagedSource, PreparedManagedMutationReceipt, ProjectKey,
};
use geosolve_sketch_engine::{
    ConstructionCommand, ConstructionEvent, ConstructionSample, ConstructionTool, EditableSession,
    MAX_CONSTRUCTION_SAMPLES, PreparedConstruction,
};

fn project() -> String {
    CodeProject::managed(
        ProjectKey("construction".into()),
        CompiledManagedSource::from_json(include_str!("fixtures/authoring-radius-2.json")).unwrap(),
    )
    .unwrap()
    .to_canonical_json()
    .unwrap()
}
fn viewport() -> Viewport {
    Viewport::new([800.0, 600.0], [0.0, 0.0], 5.0).unwrap()
}
fn case(name: &str) -> (ConstructionTool, Vec<ConstructionEvent>) {
    let tool = match name {
        "segment" | "inferred" => ConstructionTool::Segment,
        "polyline" => ConstructionTool::Polyline,
        "circle" => ConstructionTool::CenterRadiusCircle,
        "rectangle" => ConstructionTool::TwoPointAlignedRectangle,
        _ => panic!("unknown case"),
    };
    let points = match name {
        "inferred" => vec![[0.0, 0.0], [20.0, 0.1]],
        "polyline" => vec![[40.0, 40.0], [60.0, 40.0], [60.0, 60.0]],
        "circle" => vec![[40.0, 40.0], [50.0, 40.0]],
        _ => vec![[40.0, 40.0], [60.0, 50.0]],
    };
    let mut events = Vec::new();
    for position in points {
        events.push(ConstructionEvent::Move {
            position,
            suppressed: name != "inferred",
            regularized: false,
        });
        events.push(ConstructionEvent::Click {
            position,
            suppressed: name != "inferred",
            regularized: false,
        });
    }
    if name == "polyline" {
        events.push(ConstructionEvent::Complete);
    }
    (tool, events)
}
fn command(session: &EditableSession, name: &str) -> ConstructionCommand {
    let (tool, events) = case(name);
    let before = session.state();
    let mut prediction = session
        .begin_construction(tool, 71, viewport(), GeometryRole::Profile)
        .unwrap();
    let origin_scene = EditorScene::from_detached_json(&prediction.scene_json().unwrap()).unwrap();
    let mut completed = false;
    for (index, input) in events.into_iter().enumerate() {
        let frame = prediction
            .advance(
                71,
                ConstructionSample {
                    sequence: u32::try_from(index + 1).unwrap(),
                    input,
                },
            )
            .unwrap();
        assert_eq!(session.state(), before);
        assert!(frame.diagnostic.is_none(), "{name}: {:?}", frame.diagnostic);
        completed = frame.completed;
        if !completed {
            let frame_scene =
                EditorScene::from_detached_json(&prediction.scene_json().unwrap()).unwrap();
            assert_eq!(frame_scene.design_identity, origin_scene.design_identity);
            assert_eq!(
                frame_scene.accepted_revision,
                origin_scene.accepted_revision
            );
        }
    }
    assert!(completed, "{name}: native terminal was not accepted");
    let scene = EditorScene::from_detached_json(&prediction.scene_json().unwrap()).unwrap();
    assert!(!scene.curves.is_empty());
    prediction.finish(71).unwrap().command().clone()
}
fn receipt(prepared: &PreparedConstruction, name: &str) -> PreparedManagedMutationReceipt {
    let fixtures: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/construction-compilations.json")).unwrap();
    let compiled: CompiledManagedSource = serde_json::from_value(fixtures[name].clone()).unwrap();
    serde_json::from_value(
        serde_json::json!({"ticketDigest":prepared.request().ticket.ticket_digest,
        "baseSourceDigest":prepared.request().current.ir.source_digest,
        "candidateSourceDigest":compiled.ir.source_digest,"compiled":compiled}),
    )
    .unwrap()
}

#[test]
fn ordinary_construction_retains_local_drafts_and_stages_one_durable_source_transaction() {
    for name in ["segment", "polyline", "circle", "rectangle", "inferred"] {
        let client = EditableSession::open(&project(), None).unwrap();
        let command = command(&client, name);
        assert!(!command.expected_declarations.is_empty());
        let mut server = EditableSession::open(&project(), None).unwrap();
        let before = server.state();
        let prepared = server.prepare_construction(&command).unwrap();
        let staged = server
            .resolve_construction(&prepared, receipt(&prepared, name))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(server.state(), before);
        assert!(staged.result().validation.hard_residuals_validated);
        assert!(staged.result().validation.all_active_features_current);
        assert!(
            staged
                .result()
                .geometry
                .points
                .iter()
                .all(|point| point.position.into_iter().all(f64::is_finite))
        );
        independently_check_geometry(staged.result(), name);
        let cold = EditableSession::open(
            staged.project_json(),
            Some(&serde_json::to_string(staged.design()).unwrap()),
        )
        .unwrap();
        assert_eq!(
            cold.source_design_digest().unwrap(),
            staged.source_design_digest()
        );
        let expected_geometry = staged.result().geometry.clone();
        server.apply_construction_commit(staged).unwrap();
        assert_eq!(server.accepted().result().geometry, expected_geometry);
        assert_eq!(server.token().revision, before.token.revision + 1);
        assert_eq!(
            server.point_gesture_targets().unwrap(),
            cold.point_gesture_targets().unwrap()
        );
        let created = server.state();
        let token = server.token().clone();
        server.undo(&token).unwrap();
        assert_eq!(server.accepted().result().geometry, before.result.geometry);
        let token = server.token().clone();
        server.redo(&token).unwrap();
        assert_eq!(server.accepted().result().geometry, created.result.geometry);
        assert!(server.prepare_construction(&command).is_err());
    }
}

fn independently_check_geometry(result: &geosolve_sketch_engine::EngineAcceptedResult, name: &str) {
    let points = result
        .geometry
        .points
        .iter()
        .map(|point| point.position)
        .collect::<Vec<_>>();
    let expected = match name {
        "segment" => vec![[0.0, 0.0], [40.0, 40.0], [60.0, 50.0]],
        "polyline" => vec![[0.0, 0.0], [40.0, 40.0], [60.0, 40.0], [60.0, 60.0]],
        "circle" => vec![[0.0, 0.0], [40.0, 40.0]],
        "rectangle" => vec![
            [0.0, 0.0],
            [40.0, 40.0],
            [60.0, 40.0],
            [60.0, 50.0],
            [40.0, 50.0],
        ],
        "inferred" => vec![[0.0, 0.0], [20.0, 0.0]],
        _ => unreachable!(),
    };
    assert_eq!(points.len(), expected.len(), "{name}");
    for expected in expected {
        assert!(
            points
                .iter()
                .any(|point| (0..2).all(|axis| (point[axis] - expected[axis]).abs() < 1e-9)),
            "{name}: missing {expected:?}"
        );
    }
    if name == "circle" {
        assert!(
            result
                .geometry
                .scalars
                .iter()
                .any(|scalar| (scalar.value - 10.0).abs() < 1e-9)
        );
    }
    assert!(
        result
            .geometry
            .scalars
            .iter()
            .any(|scalar| (scalar.value - 2.0).abs() < 1e-9),
        "preexisting circle unchanged"
    );
}

#[test]
fn stale_forged_and_failed_construction_candidates_preserve_all_live_authority() {
    let mut session = EditableSession::open(&project(), None).unwrap();
    let command = command(&session, "segment");
    let before = session.state();
    let mut wrong = command.clone();
    wrong.expected_declarations[0].symbol = "forged".into();
    assert!(session.prepare_construction(&wrong).is_err());
    wrong = command.clone();
    wrong.basis = "foreign".into();
    assert!(session.prepare_construction(&wrong).is_err());
    wrong = command.clone();
    wrong.samples = vec![wrong.samples[0]; MAX_CONSTRUCTION_SAMPLES + 1];
    assert!(session.prepare_construction(&wrong).is_err());
    let prepared = session.prepare_construction(&command).unwrap();
    let mut forged = receipt(&prepared, "segment");
    forged.ticket_digest = "forged".into();
    assert!(session.resolve_construction(&prepared, forged).is_err());
    let staged = session
        .resolve_construction(&prepared, receipt(&prepared, "segment"))
        .unwrap();
    assert_eq!(session.state(), before);
    let competitor = session
        .resolve_construction(&prepared, receipt(&prepared, "segment"))
        .unwrap();
    session.apply_construction_commit(competitor).unwrap();
    let accepted = session.state();
    assert!(session.apply_construction_commit(staged).is_err());
    assert_eq!(session.state(), accepted);
    assert!(
        session
            .resolve_construction(&prepared, receipt(&prepared, "segment"))
            .is_err()
    );
}

#[test]
fn cancellation_invalid_routing_and_incomplete_drafts_never_publish() {
    let session = EditableSession::open(&project(), None).unwrap();
    let before = session.state();
    let mut prediction = session
        .begin_construction(
            ConstructionTool::Polyline,
            9,
            viewport(),
            GeometryRole::Construction,
        )
        .unwrap();
    let first = ConstructionSample {
        sequence: 1,
        input: ConstructionEvent::Click {
            position: [40.0, 40.0],
            suppressed: true,
            regularized: false,
        },
    };
    assert!(prediction.advance(10, first).is_err());
    let mut invalid = first;
    invalid.sequence = 2;
    assert!(prediction.advance(9, invalid).is_err());
    invalid = first;
    invalid.input = ConstructionEvent::Move {
        position: [f64::NAN, 0.0],
        suppressed: false,
        regularized: false,
    };
    assert!(prediction.advance(9, invalid).is_err());
    let frame = prediction.advance(9, first).unwrap();
    assert!(!frame.completed);
    assert!(frame.preview.is_some());
    prediction.cancel();
    assert_eq!(session.state(), before);
    let prediction = session
        .begin_construction(
            ConstructionTool::Segment,
            11,
            viewport(),
            GeometryRole::Profile,
        )
        .unwrap();
    assert!(prediction.finish(11).is_err());
    assert_eq!(session.state(), before);
}

/// Generate request inputs, then run fixtures/generate-construction.mjs with the real compiler.
#[test]
#[ignore = "explicit fixture generation; ordinary tests use checked-in real compiler outputs"]
fn write_real_compiler_requests() {
    let mut requests = serde_json::Map::new();
    for name in ["segment", "polyline", "circle", "rectangle", "inferred"] {
        let session = EditableSession::open(&project(), None).unwrap();
        let command = command(&session, name);
        let prepared = session.prepare_construction(&command).unwrap();
        requests.insert(
            name.into(),
            serde_json::to_value(prepared.request()).unwrap(),
        );
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/m98/construction-requests.json");
    std::fs::write(root, serde_json::to_string_pretty(&requests).unwrap()).unwrap();
}
