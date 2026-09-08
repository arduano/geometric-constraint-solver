// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;

#[allow(
    clippy::needless_pass_by_value,
    reason = "test command payloads are constructed inline"
)]
fn dispatch(
    bridge: &mut WorkbenchBridge,
    command: &str,
    payload: serde_json::Value,
) -> serde_json::Value {
    serde_json::from_str(
        &bridge
            .dispatch_json(
                &serde_json::json!({
                    "version": 2, "command": command, "payload": payload,
                })
                .to_string(),
            )
            .expect("dispatch"),
    )
    .expect("response")
}

fn labels(frame: &serde_json::Value) -> Vec<serde_json::Value> {
    frame["scene"]["items"]
        .as_array()
        .expect("drawing")
        .iter()
        .filter(|item| {
            item["kind"] == "text"
                && item["id"]
                    .as_str()
                    .is_some_and(|id| id.starts_with("dimension:"))
        })
        .map(|item| serde_json::json!({"id":item["id"], "position":item["position"]}))
        .collect()
}

#[test]
fn m97_bridge_wheel_settle_and_cold_selection_retain_dimension_positions() {
    let mut bridge = WorkbenchBridge::fresh().expect("bridge");
    dispatch(
        &mut bridge,
        "sample.open",
        serde_json::json!({"key":"cnc-dogbone-coupon"}),
    );
    let initial = dispatch(
        &mut bridge,
        "dimensions.mode",
        serde_json::json!({"mode":"all"}),
    );
    assert!(!labels(&initial["frame"]).is_empty());
    let persistence = bridge.persistence_contents().expect("persist");
    for _ in 0..4 {
        bridge
            .wheel_json(r#"{"version":2,"x":400,"y":300,"deltaX":0,"deltaY":-90,"ctrl":false}"#)
            .expect("wheel");
    }
    let settled = dispatch(
        &mut bridge,
        "dimensions.navigation.end",
        serde_json::Value::Null,
    );
    assert_eq!(settled["kind"], "frame");
    assert_eq!(settled["revision"], initial["revision"]);
    let before = labels(&settled["frame"]);
    assert!(!before.is_empty());
    // Exact native reproduction of the bridge's selection-only cold rebuild.
    bridge.editor_mut().set_selection(Vec::new());
    bridge.retained_scene = None;
    let after: serde_json::Value =
        serde_json::from_str(&bridge.snapshot_json().expect("snapshot")).expect("snapshot JSON");
    assert_eq!(labels(&after["frame"]), before);
    assert_eq!(bridge.persistence_contents().expect("persist"), persistence);
    let hidden = dispatch(
        &mut bridge,
        "dimensions.mode",
        serde_json::json!({"mode":"hidden"}),
    );
    assert!(labels(&hidden["frame"]).is_empty());
}

#[test]
fn m97_bridge_dimension_hover_is_frame_only_and_settle_refreshes_metadata_without_persistence() {
    let mut bridge = WorkbenchBridge::fresh().expect("bridge");
    dispatch(
        &mut bridge,
        "sample.open",
        serde_json::json!({"key":"typed-panel"}),
    );
    let persistence = bridge.persistence_contents().expect("persist");
    let revision = bridge.revision;
    for (command, payload) in [
        ("dimensions.hover", serde_json::json!({"x":5,"y":5})),
        ("dimensions.hover.clear", serde_json::Value::Null),
        ("dimensions.navigation.end", serde_json::Value::Null),
    ] {
        let response = dispatch(&mut bridge, command, payload);
        assert_eq!(response["kind"], "frame");
        assert_eq!(response["revision"], revision);
        if command == "dimensions.navigation.end" {
            assert_eq!(
                response["dimensions"],
                serde_json::to_value(bridge.dimensions_snapshot()).unwrap()
            );
            assert_eq!(response.as_object().expect("response").len(), 5);
        } else {
            assert_eq!(response.as_object().expect("response").len(), 4);
        }
        assert_eq!(bridge.persistence_contents().expect("persist"), persistence);
    }
}
