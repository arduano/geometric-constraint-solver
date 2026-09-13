// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;

fn fixture() -> (
    WorkbenchBridge,
    LocalInteraction,
    BrowsingPresentation,
    serde_json::Value,
) {
    let compiled = geosolve_sketch_code::CompiledManagedSource::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../geosolve-sketch-engine/tests/fixtures/point-gesture-constrained.json"
    )))
    .unwrap();
    let project = geosolve_sketch_code::CodeProject::managed(
        geosolve_sketch_code::ProjectKey("visibility".into()),
        compiled,
    )
    .unwrap()
    .to_canonical_json()
    .unwrap();
    let mut bridge = WorkbenchBridge::restore(&project).unwrap();
    let pair: serde_json::Value =
        serde_json::from_str(&bridge.interaction_snapshot_json().unwrap()).unwrap();
    let design: serde_json::Value =
        serde_json::from_str(&bridge.workspace_design_json().unwrap()).unwrap();
    let browsing = BrowsingPresentation::new(
        &serde_json::json!({"project":project,"design":design,"seed":pair["seed"]}).to_string(),
    )
    .unwrap();
    let local = LocalInteraction::new(&pair["seed"].to_string()).unwrap();
    (bridge, local, browsing, pair["seed"].clone())
}
fn dispatch(
    local: &mut LocalInteraction,
    command: &str,
    payload: serde_json::Value,
) -> Result<String, String> {
    let mut request = serde_json::json!({"version":2,"command":command});
    request["payload"] = payload;
    local.dispatch_json(&request.to_string())
}
#[test]
fn personal_visibility_masks_paint_and_pick_and_restores_without_model_changes() {
    let (bridge, mut local, mut browsing, seed) = fixture();
    let other = LocalInteraction::new(&seed.to_string()).unwrap();
    let project = bridge.export_project_json().unwrap();
    let design = bridge.workspace_design_json().unwrap();
    let revision = bridge.revision;
    let group = bridge.base_explorer_snapshot()[0].id.clone();
    let point = local.scene.points[1].screen_position;
    let picked = local.editor.select_pointer_item(&local.scene, point);
    assert!(picked.is_some());
    let initial = local.state();
    dispatch(
        &mut local,
        "explorer.visibility.set",
        serde_json::json!({"id":group,"visible":false}),
    )
    .unwrap();
    assert!(local.scene.points.is_empty());
    assert!(local.scene.curves.is_empty());
    assert!(matches!(
        local.editor.select_pointer_item(&local.scene, point),
        None | Some(SelectionItem::Datum(_))
    ));
    assert_eq!(
        other.editor.select_pointer_item(&other.scene, point),
        picked
    );
    assert_eq!(local.state().viewport, initial.viewport);
    assert_eq!(local.state().scene_key, initial.scene_key);
    let chrome: serde_json::Value =
        serde_json::from_str(&browsing.update_json(&local.state_json().unwrap()).unwrap()).unwrap();
    assert_eq!(chrome["explorer"][0]["effectiveVisible"], false);
    assert!(browsing.model.view().scene().points.is_empty());
    local
        .replace_json(&serde_json::json!({"seed":seed,"preserveSelection":true}).to_string())
        .unwrap();
    assert!(local.scene.points.is_empty());
    dispatch(
        &mut local,
        "explorer.visibility.isolate",
        serde_json::json!({"id":group}),
    )
    .unwrap();
    assert!(!local.scene.points.is_empty());
    let chrome: serde_json::Value =
        serde_json::from_str(&browsing.update_json(&local.state_json().unwrap()).unwrap()).unwrap();
    assert_eq!(chrome["visibilityRestoreAvailable"], true);
    dispatch(
        &mut local,
        "explorer.visibility.restore",
        serde_json::json!({}),
    )
    .unwrap();
    assert!(local.scene.points.is_empty());
    dispatch(
        &mut local,
        "explorer.visibility.set",
        serde_json::json!({"id":group,"visible":true}),
    )
    .unwrap();
    assert_eq!(
        local.editor.select_pointer_item(&local.scene, point),
        picked
    );
    dispatch(
        &mut local,
        "view.construction.toggle",
        serde_json::json!({}),
    )
    .unwrap();
    let chrome: serde_json::Value =
        serde_json::from_str(&browsing.update_json(&local.state_json().unwrap()).unwrap()).unwrap();
    assert_eq!(chrome["constructionVisible"], false);
    assert!(
        !local
            .editor
            .geometry_interaction_policy()
            .visibility
            .explicit_construction
    );
    assert_eq!(bridge.export_project_json().unwrap(), project);
    assert_eq!(bridge.workspace_design_json().unwrap(), design);
    assert_eq!(bridge.revision, revision);
    assert_eq!(browsing.export_project_json().unwrap(), project);
    assert_eq!(browsing.workspace_design_json().unwrap(), design);
}
#[test]
fn malformed_and_stale_visibility_is_transactionally_rejected() {
    let (_, mut local, mut browsing, _) = fixture();
    let initial = local.state_json().unwrap();
    assert!(
        dispatch(
            &mut local,
            "explorer.visibility.set",
            serde_json::json!({"id":"missing","visible":false})
        )
        .is_err()
    );
    assert!(
        dispatch(
            &mut local,
            "explorer.visibility.restore",
            serde_json::json!({})
        )
        .is_err()
    );
    assert_eq!(local.state_json().unwrap(), initial);
    let mut stale: serde_json::Value = serde_json::from_str(&initial).unwrap();
    stale["visibility"]["hiddenRows"] = serde_json::json!(["missing"]);
    let project = browsing.export_project_json().unwrap();
    assert!(browsing.update_json(&stale.to_string()).is_err());
    assert!(
        browsing
            .model
            .view()
            .state()
            .is_none_or(|state| state.visibility.hidden_rows.is_empty())
    );
    assert_eq!(browsing.export_project_json().unwrap(), project);
    assert!(browsing.update_json(&initial).is_ok());
}
#[test]
fn suppression_is_described_without_source_publication_and_rejects_stale_rows() {
    let (_, local, mut browsing, _) = fixture();
    let chrome: serde_json::Value =
        serde_json::from_str(&browsing.update_json(&local.state_json().unwrap()).unwrap()).unwrap();
    let row = &chrome["explorer"][0]["children"][0];
    let project = browsing.export_project_json().unwrap();
    let request = serde_json::json!({"state":local.state(),"authority":chrome["authoringDocument"]["authority"],"command":"declaration.suppression.set","payload":{"id":row["id"],"suppressed":true}});
    let mutation: serde_json::Value =
        serde_json::from_str(&browsing.describe_json(&request.to_string()).unwrap()).unwrap();
    assert_eq!(mutation["mutation"], "set_suppressed");
    assert_eq!(mutation["suppressed"], true);
    assert_eq!(mutation["target"]["target"], "declaration");
    assert_eq!(browsing.export_project_json().unwrap(), project);
    let mut stale = request;
    stale["payload"]["id"] = "missing".into();
    assert!(browsing.describe_json(&stale.to_string()).is_err());
    assert_eq!(browsing.export_project_json().unwrap(), project);
}

#[test]
fn seed_from_an_already_hidden_workbench_retains_geometry_for_local_restore() {
    let (mut bridge, _, _, _) = fixture();
    let group = bridge.base_explorer_snapshot()[0].id.clone();
    let count = bridge.retained_scene.as_ref().unwrap().points.len();
    bridge.set_explorer_row_visible(&group, false).unwrap();
    let pair: serde_json::Value =
        serde_json::from_str(&bridge.interaction_snapshot_json().unwrap()).unwrap();
    let mut local = LocalInteraction::new(&pair["seed"].to_string()).unwrap();
    assert!(local.scene.points.is_empty());
    dispatch(
        &mut local,
        "explorer.visibility.set",
        serde_json::json!({"id":group,"visible":true}),
    )
    .unwrap();
    assert_eq!(local.scene.points.len(), count);
    bridge
        .interaction_apply_json(&local.state_json().unwrap())
        .unwrap();
    assert_eq!(bridge.retained_scene.as_ref().unwrap().points.len(), count);
}
