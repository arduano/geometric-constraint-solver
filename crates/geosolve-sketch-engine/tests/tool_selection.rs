// SPDX-License-Identifier: GPL-3.0-or-later
use geosolve_constraint_editor::{
    CurvePickContext, EditorScene, SceneCurveOrigin, SelectionItem, SelectionPresentationState,
    Viewport,
};
use geosolve_sketch_code::{CodeProject, CompiledManagedSource, ProjectKey};
use geosolve_sketch_engine::{
    EditableSession, ToolCurveOccurrence, ToolOperationOperand, ToolOperationTool,
};

fn session(compiled: &str) -> EditableSession {
    named_session(compiled, "tool-selection")
}
fn named_session(compiled: &str, name: &str) -> EditableSession {
    let project = CodeProject::managed(
        ProjectKey(name.into()),
        CompiledManagedSource::from_json(compiled).unwrap(),
    )
    .unwrap();
    EditableSession::open(&project.to_canonical_json().unwrap(), None).unwrap()
}
fn viewport() -> Viewport {
    Viewport::new([800.0, 600.0], [0.0, 0.0], 5.0).unwrap()
}
fn scene(session: &EditableSession) -> EditorScene {
    let value: serde_json::Value = serde_json::from_str(
        &session
            .tool_operation_presentation_json(viewport())
            .unwrap(),
    )
    .unwrap();
    EditorScene::from_detached_json(value["scene"].as_str().unwrap()).unwrap()
}
#[test]
fn native_tool_selection_retains_exact_occurrences_and_rejects_foreign_namespaces() {
    let first = session(include_str!("fixtures/authoring-radius-2.json"));
    let second = named_session(
        include_str!("fixtures/authoring-radius-2.json"),
        "foreign-tool-selection",
    );
    let scene = scene(&first);
    let curve = &scene.curves[0];
    let selection = SelectionPresentationState {
        items: vec![SelectionItem::Curve(curve.span)],
        curve_picks: vec![CurvePickContext {
            span: curve.span,
            parameter: curve.screen_parameters[1],
            origin: curve.origin,
        }],
    };
    let before = first.state();
    let operands = first.tool_operation_operands(&selection).unwrap();
    assert!(matches!(
        operands[0],
        ToolOperationOperand::Binding {
            occurrence: Some(ToolCurveOccurrence::Native),
            ..
        }
    ));
    assert!(second.tool_operation_operands(&selection).is_err());
    let mut malformed = operands;
    if let ToolOperationOperand::Binding {
        curve_parameter, ..
    } = &mut malformed[0]
    {
        *curve_parameter = Some(1e9);
    }
    assert!(
        first
            .begin_tool_operation(ToolOperationTool::Radius, 8, viewport(), malformed)
            .is_err()
    );
    assert_eq!(first.state(), before);
}

#[test]
fn implicit_fillet_selection_keeps_source_corner_and_interval_provenance() {
    let session = session(include_str!("fixtures/point-gesture-computed.json"));
    let scene = scene(&session);
    let mut count = 0;
    for curve in &scene.curves {
        if !matches!(curve.origin, SceneCurveOrigin::FilletDiscarded { .. }) {
            continue;
        }
        count += 1;
        let parameter =
            (curve.screen_parameters[0] + curve.screen_parameters.last().unwrap()) * 0.5;
        let selection = SelectionPresentationState {
            items: vec![SelectionItem::Curve(curve.span)],
            curve_picks: vec![CurvePickContext {
                span: curve.span,
                parameter,
                origin: curve.origin,
            }],
        };
        let operands = session.tool_operation_operands(&selection).unwrap();
        assert!(matches!(
            operands[0],
            ToolOperationOperand::Binding {
                occurrence: Some(ToolCurveOccurrence::FilletDiscarded { .. }),
                ..
            }
        ));
        let mut corrupted = operands.clone();
        if let ToolOperationOperand::Binding {
            occurrence: Some(ToolCurveOccurrence::FilletDiscarded { interval, .. }),
            ..
        } = &mut corrupted[0]
        {
            interval[0] += 0.1;
        }
        assert!(
            session
                .begin_tool_operation(ToolOperationTool::Tangent, 9, viewport(), corrupted)
                .is_err()
        );
        let prediction = session
            .begin_tool_operation(ToolOperationTool::Tangent, 9, viewport(), operands)
            .unwrap();
        prediction.cancel();
    }
    assert!(
        count > 0,
        "fixture must exercise computed discarded intervals"
    );
}
