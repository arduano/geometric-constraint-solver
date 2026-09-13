// SPDX-License-Identifier: GPL-3.0-or-later
use geosolve_constraint_editor::{EditorScene, Viewport};
use geosolve_sketch_code::{CodeProject, CompiledManagedSource, ProjectKey};
use geosolve_sketch_engine::EditableSession;

#[test]
fn accepted_inspection_keeps_source_native_identity_and_history_unchanged() {
    let compiled =
        CompiledManagedSource::from_json(include_str!("fixtures/point-gesture-constrained.json"))
            .unwrap();
    let project = CodeProject::managed(ProjectKey("inspection".into()), compiled)
        .unwrap()
        .to_canonical_json()
        .unwrap();
    let session = EditableSession::open(&project, None).unwrap();
    let before = (
        session.state(),
        session.export_project_json().unwrap(),
        session.design(),
    );
    let view = Viewport::new([800.0, 600.0], [0.0, 0.0], 12.0).unwrap();
    let inspection = session.inspect(view).unwrap();
    assert_eq!(inspection.token, before.0.token);
    assert_eq!(inspection.accepted.result_id, before.0.result.result_id);
    let scene = EditorScene::from_detached_json(&inspection.accepted.scene).unwrap();
    assert_eq!(
        scene.presentation_document().id(),
        inspection.accepted.bindings.document
    );
    assert_eq!(scene.viewport, view);
    assert_eq!(
        scene.presentation_document().points(),
        before.0.result.geometry.points
    );
    assert!(inspection.navigation.blocked_reason.is_none());
    assert!(
        inspection
            .navigation
            .entries
            .iter()
            .any(|row| !row.nodes.is_empty())
    );
    let invalid = Viewport {
        pixels_per_model_unit: f64::NAN,
        ..view
    };
    assert!(session.inspect(invalid).is_err());
    let seed = session.interaction_seed(None).unwrap();
    let local = geosolve_constraint_editor::detached_interaction::DetachedCanvas::new(
        &serde_json::to_string(&seed).unwrap(),
    )
    .unwrap();
    assert_eq!(
        local.scene.presentation_document().points(),
        before.0.result.geometry.points
    );
    assert_eq!(seed.scene_key, before.0.result.result_id);
    assert_eq!(seed.point_targets.len(), 2);
    assert!(!seed.presence_bindings.is_empty());
    assert!(seed.dimensions.ids.is_empty());
    assert!(!seed.host_size_received);
    let sized = session
        .interaction_seed(Some(geosolve_sketch_engine::InteractionViewport {
            width: 900.0,
            height: 450.0,
            pixel_ratio: 2.0,
        }))
        .unwrap();
    assert!(sized.host_size_received);
    assert_eq!(
        EditorScene::from_detached_json(&sized.scene)
            .unwrap()
            .viewport
            .screen_size
            .map(f64::to_bits),
        [900.0_f64, 450.0_f64].map(f64::to_bits)
    );
    for extent in [f64::NAN, f64::INFINITY, 0.0, -1.0, 32_769.0] {
        assert!(
            session
                .interaction_seed(Some(geosolve_sketch_engine::InteractionViewport {
                    width: extent,
                    height: 450.0,
                    pixel_ratio: 1.0,
                }))
                .is_err()
        );
    }
    assert_eq!(
        (
            session.state(),
            session.export_project_json().unwrap(),
            session.design()
        ),
        before
    );
}

#[test]
fn detached_selection_preserves_visibility_and_exact_scene_authority() {
    let compiled =
        CompiledManagedSource::from_json(include_str!("fixtures/point-gesture-constrained.json"))
            .unwrap();
    let project = CodeProject::managed(ProjectKey("selection-inspection".into()), compiled)
        .unwrap()
        .to_canonical_json()
        .unwrap();
    let session = EditableSession::open(&project, None).unwrap();
    let viewport = Viewport::new([800.0, 600.0], [0.0, 0.0], 12.0).unwrap();
    let inspection = session.inspect(viewport).unwrap();
    let scene = EditorScene::from_detached_json(&inspection.accepted.scene).unwrap();
    let curve = &scene.curves[0];
    let selected = vec![geosolve_constraint_editor::SelectionItem::Curve(curve.span)];
    let pick = geosolve_constraint_editor::CurvePickContext {
        span: curve.span,
        parameter: (curve.screen_parameters.first().unwrap()
            + curve.screen_parameters.last().unwrap())
            * 0.5,
        origin: curve.origin,
    };
    let mut view = serde_json::json!({
        "seed": {"format":"geosolve-local-interaction-v1","sceneKey":"accepted-1",
            "scene":inspection.accepted.scene,"bindings":inspection.accepted.bindings,
            "visibilitySeed":{"rows":{"selected":{"ancestors":[],"items":selected,"isolate":false}}},
            "visibility":{"hiddenRows":[],"isolateRestore":null,"constructionVisible":true}},
        "state":{"format":"geosolve-local-interaction-v1","sceneKey":"accepted-1","selection":selected,"curvePicks":[pick]}
    });
    let before = session.state();
    assert_eq!(
        session
            .tool_operation_view_operands(viewport, serde_json::from_value(view.clone()).unwrap())
            .unwrap()
            .len(),
        1
    );
    view["seed"]["visibility"]["hiddenRows"] = serde_json::json!(["selected"]);
    assert!(
        session
            .tool_operation_view_operands(viewport, serde_json::from_value(view.clone()).unwrap())
            .is_err()
    );
    view["seed"]["visibility"]["hiddenRows"] = serde_json::json!([]);
    view["state"]["sceneKey"] = serde_json::json!("obsolete");
    assert!(
        session
            .tool_operation_view_operands(viewport, serde_json::from_value(view).unwrap())
            .is_err()
    );
    assert_eq!(session.state(), before);
}
