// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;

fn sample(key: &str) -> WorkbenchBridge {
    let mut bridge = WorkbenchBridge::construct_json(r#"{"version":2}"#).unwrap();
    bridge.open_sample(key, None).unwrap();
    bridge.refresh_current_scene();
    bridge
}

#[test]
fn m97_dimension_overview_and_pins_preserve_design_and_reload_exactly() {
    let mut bridge = sample("dust-shoe-clamp");
    let before = bridge
        .code_project
        .as_ref()
        .unwrap()
        .to_persistence_json()
        .unwrap();
    let overview = bridge.dimensions_snapshot();
    assert!(!overview.entries.is_empty());
    assert!(overview.entries.iter().all(|row| row.default_priority));
    assert_eq!(overview.pin_count, 0);
    assert!(bridge.dimensions.entries.len() >= 5);
    let ids: Vec<_> = bridge
        .dimensions
        .entries
        .iter()
        .take(5)
        .map(|(_, row)| row.id.clone())
        .collect();
    for id in &ids[..4] {
        bridge
            .dispatch_dimensions(
                "dimensions.pin",
                serde_json::json!({"id": id, "pinned": true}),
            )
            .unwrap();
    }
    assert!(
        bridge
            .dispatch_dimensions(
                "dimensions.pin",
                serde_json::json!({"id": ids[4], "pinned": true})
            )
            .is_err()
    );
    bridge.refresh_current_scene();
    assert_eq!(bridge.dimensions_snapshot().pin_count, 4);
    assert_eq!(
        bridge
            .code_project
            .as_ref()
            .unwrap()
            .to_persistence_json()
            .unwrap(),
        before
    );
    let persistence: serde_json::Value =
        serde_json::from_str(&bridge.persistence_json().unwrap()).unwrap();
    let mut restored = WorkbenchBridge::construct_json(
        &serde_json::json!({"version":2, "persistedProject":persistence["contents"]}).to_string(),
    )
    .unwrap();
    restored.refresh_current_scene();
    assert_eq!(restored.dimensions_snapshot().pin_count, 4);
    assert_eq!(
        restored.dimension_persistence().pins,
        bridge.dimension_persistence().pins
    );
    restored
        .dispatch_dimensions("dimensions.clearPins", serde_json::Value::Null)
        .unwrap();
    assert_eq!(restored.dimensions_snapshot().pin_count, 0);
}

#[test]
fn m97_selected_channel_exposes_full_width_and_all_related_measurements() {
    let mut bridge = sample("pc-water-manifold");
    assert!(bridge.dimensions.entries.len() > 50);
    assert!(
        bridge
            .dimensions
            .entries
            .iter()
            .all(|(_, row)| !row.label.starts_with("code."))
    );
    let code = bridge.code_project.as_ref().unwrap();
    let panel = code.declaration_panel_projection(bridge.editor());
    let channel = panel
        .declarations
        .iter()
        .find(|declaration| {
            declaration.kind == "Patch invocation" && declaration.label.contains("upper")
        })
        .unwrap();
    let navigation = code.navigation_index(bridge.editor());
    let nodes = navigation
        .entries
        .iter()
        .find(|entry| entry.id == channel.id)
        .unwrap()
        .nodes
        .clone();
    let items = bridge.editor().navigation_selection_items(nodes);
    bridge.editor_mut().set_selection(items);
    bridge.refresh_current_scene();
    let snapshot = bridge.dimensions_snapshot();
    assert!(!snapshot.entries.is_empty());
    assert!(snapshot.entries.iter().any(|entry| entry.generated));
    assert!(
        snapshot
            .entries
            .iter()
            .filter(|entry| entry.generated)
            .all(|entry| entry.label.starts_with("upperChannel"))
    );
    assert!(
        snapshot
            .entries
            .iter()
            .filter(|entry| entry.visible && !entry.default_priority)
            .count()
            <= 6
    );
    assert!(
        snapshot.parameters.iter().any(|parameter| parameter
            .label
            .to_lowercase()
            .contains("width")
            && parameter.value == "12"),
        "{snapshot:?}"
    );
    let (width_item, width_control) = bridge
        .dimensions
        .entries
        .iter()
        .find_map(|(entry, row)| {
            (row.label == "reservoirWidth").then(|| {
                let metadata = &bridge.dimensions.cache.dimensions[&entry.key.item];
                let DimensionEdit::Source { control, .. } = &metadata.edit else {
                    panic!("accepted width source edit");
                };
                (entry.key.item, control.clone())
            })
        })
        .expect("accepted declaration supplies the friendly dimension name");
    bridge.editor_mut().set_selection([width_item]);
    bridge.refresh_current_scene();
    let width_snapshot = bridge.dimensions_snapshot();
    assert_eq!(
        width_snapshot
            .entries
            .iter()
            .filter(|entry| entry.label == "reservoirWidth")
            .count(),
        1
    );
    assert!(
        !width_snapshot
            .parameters
            .iter()
            .any(|parameter| parameter.id == width_control),
        "ordinary dimension targets appear once, in the measurement row"
    );
    let all = bridge.dimensions.entries.len();
    bridge
        .dispatch_dimensions("dimensions.mode", serde_json::json!({"mode":"all"}))
        .unwrap();
    bridge.refresh_current_scene();
    assert_eq!(bridge.dimensions.entries.len(), all);
}

#[test]
fn m97_priority_gridfinity_preserves_every_authored_measurement_and_hidden_mode() {
    let mut bridge = sample("gridfinity-bin-section");
    let before = bridge
        .code_project
        .as_ref()
        .unwrap()
        .to_persistence_json()
        .unwrap();
    let snapshot = bridge.dimensions_snapshot();
    assert_eq!(snapshot.entries.len(), 20);
    assert!(
        snapshot
            .entries
            .iter()
            .all(|row| row.default_priority && !row.generated && !row.pinned)
    );
    assert!(snapshot.entries.iter().any(|row| row.visible));
    assert!(snapshot.entries.iter().any(|row| row.reference));
    assert_eq!(snapshot.pin_count, 0);
    let keys: Vec<_> = bridge
        .dimensions
        .entries
        .iter()
        .map(|(entry, _)| entry.key)
        .collect();
    bridge
        .dispatch_dimensions("dimensions.mode", serde_json::json!({"mode":"hidden"}))
        .unwrap();
    bridge.refresh_current_scene();
    let hidden = bridge.dimensions_snapshot();
    assert_eq!(
        hidden.entries.len(),
        20,
        "occluded/hidden priorities stay inspectable"
    );
    assert!(hidden.entries.iter().all(|row| !row.visible));
    assert_eq!(
        bridge
            .code_project
            .as_ref()
            .unwrap()
            .to_persistence_json()
            .unwrap(),
        before
    );
    let persistence: serde_json::Value =
        serde_json::from_str(&bridge.persistence_json().unwrap()).unwrap();
    let mut restored = WorkbenchBridge::construct_json(
        &serde_json::json!({"version":2, "persistedProject":persistence["contents"]}).to_string(),
    )
    .unwrap();
    restored.refresh_current_scene();
    assert!(matches!(
        restored.dimensions_snapshot().mode,
        DisplayMode::Hidden
    ));
    assert_eq!(restored.dimensions_snapshot().entries.len(), 20);
    assert_eq!(
        restored
            .dimensions
            .entries
            .iter()
            .map(|(entry, _)| entry.key)
            .collect::<Vec<_>>(),
        keys
    );
    restored
        .dispatch_dimensions("dimensions.mode", serde_json::json!({"mode":"focused"}))
        .unwrap();
    restored.refresh_current_scene();
    assert!(
        restored
            .dimensions_snapshot()
            .entries
            .iter()
            .any(|row| row.visible)
    );
    assert!(restored.dimension_persistence().pins.is_empty());
}

#[test]
fn m97_priority_manifold_keeps_design_sizes_and_truthful_public_widths() {
    let mut bridge = sample("pc-water-manifold");
    let before = bridge
        .code_project
        .as_ref()
        .unwrap()
        .to_persistence_json()
        .unwrap();
    let snapshot = bridge.dimensions_snapshot();
    assert_eq!(snapshot.entries.len(), 6);
    assert!(
        snapshot
            .entries
            .iter()
            .all(|row| row.default_priority && !row.generated && !row.pinned)
    );
    for (label, value) in [
        ("plateWidth", "240"),
        ("plateHeight", "120"),
        ("reservoirWidth", "60"),
        ("reservoirHeight", "84"),
    ] {
        assert!(
            snapshot
                .entries
                .iter()
                .any(|row| row.label == label && row.value == value),
            "{label}: {snapshot:?}"
        );
    }
    assert!(
        snapshot
            .parameters
            .iter()
            .any(|row| row.label == "Channel width"
                && row.value == "12"
                && row.default_priority
                && row.editable)
    );
    assert!(
        snapshot
            .parameters
            .iter()
            .any(|row| row.label == "Seal groove width"
                && row.value == "2.4"
                && row.default_priority
                && row.editable)
    );
    assert_eq!(snapshot.pin_count, 0);
    assert!(snapshot.entries.iter().any(|row| row.visible));
    bridge
        .dispatch_dimensions("dimensions.clearPins", serde_json::Value::Null)
        .unwrap();
    bridge.refresh_current_scene();
    assert_eq!(bridge.dimensions_snapshot().entries.len(), 6);
    assert_eq!(
        bridge
            .code_project
            .as_ref()
            .unwrap()
            .to_persistence_json()
            .unwrap(),
        before
    );
}

#[test]
fn m97_stale_dimension_commands_and_draft_edits_are_rejected() {
    let mut bridge = sample("dust-shoe-clamp");
    let labels: Vec<_> = bridge
        .dimensions
        .entries
        .iter()
        .map(|(_, row)| row.label.clone())
        .collect();
    let old_id = bridge.dimensions.entries.first().unwrap().1.id.clone();
    bridge
        .dispatch_dimensions(
            "dimensions.pin",
            serde_json::json!({"id":old_id, "pinned":true}),
        )
        .unwrap();
    let source = bridge
        .code_project
        .as_ref()
        .unwrap()
        .managed_draft()
        .to_owned();
    bridge
        .code_project
        .as_mut()
        .unwrap()
        .set_managed_draft(format!("{source}\n// unapplied"));
    assert!(
        bridge
            .dispatch_dimensions("dimensions.focus", serde_json::json!({"id":old_id}))
            .is_err()
    );
    bridge.refresh_current_scene();
    assert_eq!(
        bridge
            .dimensions
            .entries
            .iter()
            .map(|(_, row)| row.label.clone())
            .collect::<Vec<_>>(),
        labels,
        "friendly names remain bound to accepted provenance beneath a draft"
    );
    assert!(
        bridge
            .dimensions
            .entries
            .iter()
            .all(|(_, entry)| !entry.editable)
    );
    let persisted = bridge.dimension_persistence();
    bridge.open_sample("typed-panel", None).unwrap();
    bridge.restore_dimensions(persisted).unwrap();
    bridge.refresh_current_scene();
    assert_eq!(bridge.dimensions_snapshot().pin_count, 0);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one exact target witnesses edit, stale rejection, history, reload and precision"
)]
fn m97_native_dimension_edit_uses_inspector_history_and_invalidates_old_tokens() {
    let mut bridge = WorkbenchBridge::construct_json(r#"{"version":2}"#).unwrap();
    bridge.open_sample("dust-shoe-clamp", None).unwrap();
    // Exercise the supported plain projectional host, without managed source.
    bridge.code_project = None;
    bridge.refresh_current_scene();
    assert_eq!(
        bridge
            .dimensions_snapshot()
            .entries
            .iter()
            .filter(|row| row.default_priority)
            .count(),
        0,
        "native sketches without source metadata do not infer overview intent from order"
    );
    let before = bridge.editor().coordinator().intent().identity();
    let revision_before = bridge.revision;
    let (key, row) = bridge
        .dimensions
        .entries
        .iter()
        .find(|(_, row)| row.editable && row.kind == "Radius")
        .map(|(entry, row)| (entry.key, row.clone()))
        .expect("native editable radius");
    let next = row.value.parse::<f64>().unwrap() + 1.0009;
    bridge
        .dispatch_dimensions(
            "dimensions.pin",
            serde_json::json!({"id":row.id, "pinned":true}),
        )
        .unwrap();
    bridge
        .dispatch_dimensions(
            "dimensions.edit",
            serde_json::json!({"id":row.id, "value":next}),
        )
        .unwrap();
    bridge.refresh_current_scene();
    assert_ne!(bridge.editor().coordinator().intent().identity(), before);
    assert_eq!(bridge.revision, revision_before + 1);
    let edited = bridge
        .dimensions
        .entries
        .iter()
        .find(|(entry, _)| entry.key == key)
        .unwrap();
    assert!((edited.1.value.parse::<f64>().unwrap() - next).abs() < 1e-9);
    assert!(edited.1.pinned);
    assert_eq!(edited.1.row_key, row.row_key);
    assert_eq!(
        serde_json::to_value(&edited.1).unwrap()["rowKey"],
        row.row_key
    );
    assert_ne!(edited.1.id, row.id);
    assert!(
        bridge
            .dispatch_dimensions(
                "dimensions.edit",
                serde_json::json!({"id":row.id, "value":next+1.0})
            )
            .is_err()
    );
    bridge
        .editor_mut()
        .undo()
        .unwrap()
        .expect("one dimension edit history entry");
    bridge.refresh_current_scene();
    assert_eq!(
        bridge
            .dimensions
            .entries
            .iter()
            .find(|(entry, _)| entry.key == key)
            .unwrap()
            .1
            .value,
        row.value
    );
    assert_eq!(bridge.dimensions.native.pins, vec![key]);
    bridge.editor_mut().redo().unwrap().expect("redo dimension");
    bridge.refresh_current_scene();
    let mut restored = WorkbenchBridge::restore(&bridge.persistence_contents().unwrap()).unwrap();
    restored.refresh_current_scene();
    let precise = restored
        .dimensions
        .entries
        .iter()
        .find(|(entry, _)| entry.key == key)
        .unwrap()
        .1
        .clone();
    assert_eq!(
        precise.value.parse::<f64>().unwrap().to_bits(),
        next.to_bits()
    );
    let rounded = next.floor();
    restored
        .dispatch_dimensions(
            "dimensions.edit",
            serde_json::json!({"id":precise.id,"value":rounded}),
        )
        .unwrap();
    restored.refresh_current_scene();
    assert_eq!(
        restored
            .dimensions
            .entries
            .iter()
            .find(|(entry, _)| entry.key == key)
            .unwrap()
            .1
            .value
            .parse::<f64>()
            .unwrap()
            .to_bits(),
        rounded.to_bits()
    );
}

#[test]
fn m97_source_dimension_edit_prepares_existing_managed_transaction() {
    let mut bridge = WorkbenchBridge::construct_json(r#"{"version":2}"#).unwrap();
    bridge.open_sample("dust-shoe-clamp", None).unwrap();
    bridge.refresh_current_scene();
    let row = bridge
        .dimensions
        .entries
        .iter()
        .find(|(_, row)| row.editable && row.kind == "Radius")
        .map(|(_, row)| row.clone())
        .expect("managed editable radius");
    let before = bridge
        .code_project
        .as_ref()
        .unwrap()
        .to_persistence_json()
        .unwrap();
    let next = row.value.parse::<f64>().unwrap() + 1.0;
    bridge
        .dispatch_dimensions(
            "dimensions.edit",
            serde_json::json!({"id":row.id, "value":next}),
        )
        .unwrap();
    assert!(
        bridge.pending_managed_mutation.is_some(),
        "source edits must prepare the existing compiler transaction"
    );
    let pending =
        pending_managed_ticket_digest(bridge.pending_managed_mutation.as_ref().unwrap()).to_owned();
    for command in ["dimensions.navigation.end", "dimensions.hover.clear"] {
        let response = bridge
            .dispatch_json(&serde_json::json!({"version":2,"command":command}).to_string())
            .unwrap();
        assert_eq!(
            response, "null",
            "late presentation cleanup must retain the pending compiler snapshot"
        );
        assert_eq!(
            pending_managed_ticket_digest(bridge.pending_managed_mutation.as_ref().unwrap()),
            pending
        );
    }
    assert_eq!(
        bridge
            .code_project
            .as_ref()
            .unwrap()
            .to_persistence_json()
            .unwrap(),
        before,
        "uncompiled source edits cannot publish native-only geometry"
    );
}
