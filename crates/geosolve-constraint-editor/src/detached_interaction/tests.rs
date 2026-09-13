// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;

fn seeded() -> (DetachedCanvas, String) {
    let (document, _, _) = crate::tests::line_document();
    let scene = crate::tests::scene(&document);
    let seed = DetachedInteractionSeed {
        format: FORMAT.into(),
        scene_key: "native-detached-fixture".into(),
        scene: scene.to_detached_json().unwrap(),
        title: "Native lines".into(),
        host_size_received: true,
        semantic_preview: false,
        selection: vec![],
        curve_picks: vec![],
        grid_visible: true,
        policy: GeometryInteractionPolicy::default(),
        dimensions: LocalDimensionSeed {
            state: DimensionPresentationState::default(),
            context: crate::DimensionPresentationContext::default(),
            layout: crate::AnnotationLayoutState::default(),
            ids: BTreeMap::new(),
        },
        visibility_seed: VisibilitySeed::default(),
        visibility: VisibilityState::default(),
        bindings: None,
        point_targets: BTreeMap::new(),
        presence_bindings: BTreeMap::new(),
    };
    let encoded = serde_json::to_string(&seed).unwrap();
    (DetachedCanvas::new(&encoded).unwrap(), encoded)
}

#[test]
fn invalid_late_wheel_and_resize_preserve_complete_personal_view() {
    let (mut local, _) = seeded();
    let state = local.state_json().unwrap();
    let malformed = r#"{"version":2,"samples":[{"version":2,"x":20.0,"y":20.0,"deltaX":0.0,"deltaY":-50.0,"ctrl":false},{"version":2,"x":-1.0,"y":20.0,"deltaX":0.0,"deltaY":-50.0,"ctrl":false}]}"#;
    assert!(local.wheel_json(malformed).is_err());
    assert_eq!(local.state_json().unwrap(), state);
    assert!(
        local
            .resize_json(r#"{"version":2,"width":-1.0,"height":670.0,"pixelRatio":2.0}"#)
            .is_err()
    );
    assert_eq!(local.state_json().unwrap(), state);
}

#[test]
fn captured_pan_and_replacement_preserve_accepted_geometry() {
    let (mut local, seed) = seeded();
    let authority = local.full_scene.to_detached_json().unwrap();
    let origin = local.state().viewport;
    let pointer = |phase, id, buttons, x, y| {
        serde_json::json!({
            "version":2,"phase":phase,"pointerId":id,"buttons":buttons,"x":x,"y":y,
            "modifiers":{"alt":false,"ctrl":false,"meta":false,"shift":false}
        })
        .to_string()
    };
    local
        .pointer_json(&pointer("down", 7, 4, 200.0, 200.0))
        .unwrap();
    let before = local.state();
    assert!(
        local
            .pointer_json(&pointer("move", 8, 4, 220.0, 230.0))
            .is_err()
    );
    assert_eq!(local.state(), before);
    local
        .pointer_json(&pointer("up", 7, 0, -20.0, 230.0))
        .unwrap();
    let current = local.state().viewport;
    assert!(
        (current.model_center[0] - origin.model_center[0] - 220.0 / origin.pixels_per_model_unit)
            .abs()
            < 1e-12
    );
    assert!(
        (current.model_center[1] - origin.model_center[1] - 30.0 / origin.pixels_per_model_unit)
            .abs()
            < 1e-12
    );
    assert!(local.pan.is_none());
    assert_eq!(local.full_scene.to_detached_json().unwrap(), authority);
    let seed: serde_json::Value = serde_json::from_str(&seed).unwrap();
    let (replacement, update) = local
        .prepare_replacement(&serde_json::json!({"seed":seed,"preserveSelection":true}).to_string())
        .unwrap();
    assert_eq!(replacement.state(), local.state());
    assert_eq!(update.state, local.state());
    assert_eq!(
        replacement.full_scene.to_detached_json().unwrap(),
        authority
    );
}

#[test]
fn navigation_selection_requires_exact_view_and_valid_occurrence() {
    let (mut local, _) = seeded();
    let expected = local.state();
    let curve = &local.scene.curves[0];
    let mut state = expected.clone();
    state.selection = vec![SelectionItem::Curve(curve.span)];
    state.curve_picks = vec![CurvePickContext {
        span: curve.span,
        parameter: 0.5,
        origin: curve.origin,
    }];
    let mut invalid = state.clone();
    invalid.curve_picks[0].parameter = 1e200;
    assert!(
        local
            .restore_selection_json(
                &serde_json::json!({"expected":expected,"state":invalid}).to_string()
            )
            .is_err()
    );
    assert_eq!(local.state(), expected);
    let request = serde_json::json!({"expected":expected,"state":state}).to_string();
    assert!(
        local
            .restore_selection_json(&request)
            .unwrap()
            .unwrap()
            .selection_changed
    );
    assert_eq!(local.state(), state);
    assert!(local.restore_selection_json(&request).is_err());
    assert_eq!(local.state(), state);
    let mut invalid = state.clone();
    invalid.grid_visible = !invalid.grid_visible;
    assert!(
        local
            .restore_selection_json(
                &serde_json::json!({"expected":state,"state":invalid}).to_string()
            )
            .is_err()
    );
    assert_eq!(local.state(), state);
}
