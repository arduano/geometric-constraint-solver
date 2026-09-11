// SPDX-License-Identifier: GPL-3.0-or-later
use geosolve_constraint_editor::{SelectionItem, Viewport};
use geosolve_sketch_code::{CodeProject, CompiledManagedSource, ProjectKey};
use geosolve_sketch_engine::{
    EditableSession, ToolOperationEvent, ToolOperationSample, ToolOperationTool,
};

fn session() -> EditableSession {
    let project = CodeProject::managed(
        ProjectKey("tool-paint".into()),
        CompiledManagedSource::from_json(include_str!("fixtures/tool-operation-feature.json"))
            .unwrap(),
    )
    .unwrap();
    EditableSession::open(&project.to_canonical_json().unwrap(), None).unwrap()
}
fn viewport() -> Viewport {
    Viewport::new([800.0, 600.0], [0.0, 0.0], 5.0).unwrap()
}
#[test]
fn native_operation_paint_retains_pending_hover_and_offset_chain() {
    let session = session();
    let before = session.state();
    let curve = session.accepted().result().geometry.curves[0].curve.id;
    let span = geosolve_sketch::CurveSpan { curve, segment: 0 };
    let operand = session
        .tool_operation_operand(SelectionItem::Curve(span), Some(0.5))
        .unwrap();
    let mut prediction = session
        .begin_tool_operation(
            ToolOperationTool::Parallel,
            101,
            viewport(),
            vec![operand.clone()],
        )
        .unwrap();
    let paint: serde_json::Value =
        serde_json::from_str(&prediction.presentation_json().unwrap()).unwrap();
    assert_eq!(
        paint["operation"]["pending"],
        serde_json::json!([SelectionItem::Curve(span)])
    );
    let scene = geosolve_constraint_editor::EditorScene::from_detached_json(
        &prediction.scene_json().unwrap(),
    )
    .unwrap();
    let next = scene
        .curves
        .iter()
        .find(|curve| curve.span.segment == 1)
        .unwrap();
    let screen = next.screen_polyline[next.screen_polyline.len() / 2];
    let position = scene.viewport.screen_to_model(screen);
    prediction
        .advance(
            101,
            ToolOperationSample {
                sequence: 1,
                input: ToolOperationEvent::Move { position },
            },
        )
        .unwrap();
    let paint: serde_json::Value =
        serde_json::from_str(&prediction.presentation_json().unwrap()).unwrap();
    assert_eq!(
        paint["operation"]["hover"],
        serde_json::json!(SelectionItem::Curve(next.span))
    );
    prediction.cancel();

    let mut prediction = session
        .begin_tool_operation(ToolOperationTool::Offset, 102, viewport(), vec![operand])
        .unwrap();
    prediction
        .advance(
            102,
            ToolOperationSample {
                sequence: 1,
                input: ToolOperationEvent::OffsetDistance { distance: 2.0 },
            },
        )
        .unwrap();
    let paint: serde_json::Value =
        serde_json::from_str(&prediction.presentation_json().unwrap()).unwrap();
    assert!(
        !paint["operation"]["provisional"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let chain = &paint["operation"]["offset"]["chain"];
    assert_eq!(chain["spans"].as_array().unwrap().len(), 1);
    assert_ne!(
        chain["start"]["model_position"],
        chain["end"]["model_position"]
    );
    assert_eq!(
        paint["operation"]["offset"]["pending"],
        serde_json::json!([SelectionItem::Curve(span)])
    );
    prediction.cancel();
    assert_eq!(session.state(), before);
}
