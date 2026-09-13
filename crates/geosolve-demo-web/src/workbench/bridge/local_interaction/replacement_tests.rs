// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;

fn fixture() -> (
    WorkbenchBridge,
    WorkbenchBridge,
    LocalInteraction,
    serde_json::Value,
) {
    fixture_from(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../geosolve-sketch-engine/tests/fixtures/point-gesture-constrained.json"
    )))
}
fn fixture_from(
    compiled: &str,
) -> (
    WorkbenchBridge,
    WorkbenchBridge,
    LocalInteraction,
    serde_json::Value,
) {
    let compiled = geosolve_sketch_code::CompiledManagedSource::from_json(compiled).unwrap();
    let project = geosolve_sketch_code::CodeProject::managed(
        geosolve_sketch_code::ProjectKey("replacement".into()),
        compiled,
    )
    .unwrap()
    .to_canonical_json()
    .unwrap();
    let mut first = WorkbenchBridge::restore(&project).unwrap();
    let mut second = WorkbenchBridge::restore(&project).unwrap();
    let first_pair: serde_json::Value =
        serde_json::from_str(&first.interaction_snapshot_json().unwrap()).unwrap();
    let second_pair: serde_json::Value =
        serde_json::from_str(&second.interaction_snapshot_json().unwrap()).unwrap();
    let local = LocalInteraction::new(&first_pair["seed"].to_string()).unwrap();
    (first, second, local, second_pair["seed"].clone())
}

fn set_selection(local: &mut LocalInteraction, selection: SelectionPresentationState) {
    let expected = local.state();
    let mut state = expected.clone();
    state.selection = selection.items;
    state.curve_picks = selection.curve_picks;
    local
        .restore_selection_json(&serde_json::json!({"expected":expected,"state":state}).to_string())
        .unwrap();
}

fn select_and_pin(local: &mut LocalInteraction) {
    let point = local.scene.points[1].screen_position;
    local.pointer_json(&serde_json::json!({"version":2,"phase":"down","pointerId":1,"x":point.x,"y":point.y,"buttons":1,"modifiers":{"alt":false,"ctrl":false,"meta":false,"shift":false}}).to_string()).unwrap();
    let point_selection = local.editor.selection()[0];
    let curve = local.scene.curves[0].span;
    set_selection(
        local,
        SelectionPresentationState {
            items: vec![point_selection, SelectionItem::Curve(curve)],
            curve_picks: vec![CurvePickContext {
                span: curve,
                parameter: 0.5,
                origin: geosolve_constraint_editor::SceneCurveOrigin::Native,
            }],
        },
    );
    let dimension = local.dimensions.ids.keys().next().unwrap().clone();
    for (command, payload) in [
        (
            "dimensions.pin",
            serde_json::json!({"id":dimension,"pinned":true}),
        ),
        ("dimensions.focus", serde_json::json!({"id":dimension})),
    ] {
        local
            .dispatch_json(
                &serde_json::json!({"version":2,"command":command,"payload":payload}).to_string(),
            )
            .unwrap();
    }
    local
        .wheel_json(r#"{"version":2,"x":300,"y":250,"deltaX":0,"deltaY":-30,"ctrl":false}"#)
        .unwrap();
}

#[test]
fn authoring_selection_maps_exact_source_curve_occurrences_between_engine_namespaces() {
    let (mut first, second, mut local, seed) = fixture();
    select_and_pin(&mut local);
    let pair: serde_json::Value =
        serde_json::from_str(&first.interaction_snapshot_json().unwrap()).unwrap();
    let request = serde_json::json!({
        "scene": seed["scene"], "bindings": seed["bindings"],
        "view": { "seed": pair["seed"], "state": local.state() }
    });
    let before = (
        first.export_project_json().unwrap(),
        second.export_project_json().unwrap(),
    );
    let destination = LocalInteraction::new(&seed.to_string()).unwrap();
    let map = |request: &serde_json::Value| {
        serde_json::from_value::<geosolve_constraint_editor::DetachedSelectionView>(
            request["view"].clone(),
        )
        .unwrap()
        .map_to(&destination.scene, destination.bindings.as_ref())
    };
    let mapped = map(&request).unwrap();
    assert_eq!(
        mapped.items,
        vec![
            SelectionItem::Point(destination.scene.points[1].id),
            SelectionItem::Curve(destination.scene.curves[0].span)
        ]
    );
    assert_eq!(
        mapped.curve_picks,
        vec![CurvePickContext {
            span: destination.scene.curves[0].span,
            parameter: 0.5,
            origin: geosolve_constraint_editor::SceneCurveOrigin::Native
        }]
    );
    assert_eq!(
        before,
        (
            first.export_project_json().unwrap(),
            second.export_project_json().unwrap()
        )
    );
    let mut stale = request.clone();
    stale["view"]["state"]["sceneKey"] = "obsolete".into();
    assert!(map(&stale).is_err());
    let mut invalid = request;
    invalid["view"]["state"]["curvePicks"][0]["parameter"] = 2.0.into();
    assert!(map(&invalid).is_err());
}

#[test]
fn replacement_reconciles_selection_curve_occurrence_and_dimension_preferences_across_native_namespaces()
 {
    let (first, second, mut local, seed) = fixture();
    let original = (
        first.export_project_json().unwrap(),
        first.workspace_design_json().unwrap(),
        second.export_project_json().unwrap(),
        second.workspace_design_json().unwrap(),
    );
    select_and_pin(&mut local);
    let before = local.state();
    assert_eq!(before.selection.len(), 2);
    assert_eq!(before.dimension_pins.len(), 1);
    assert!(before.dimension_focus.is_some());
    let next = LocalInteraction::new(&seed.to_string()).unwrap();
    assert_ne!(
        local.scene.presentation_document().id(),
        next.scene.presentation_document().id()
    );
    let expected_point = next.scene.points[1].id;
    let expected_curve = next.scene.curves[0].span;
    let expected_dimension = *next.dimensions.ids.values().next().unwrap();
    local
        .replace_json(&serde_json::json!({"seed":seed,"preserveSelection":true}).to_string())
        .unwrap();
    assert_eq!(
        local.editor.selection(),
        &[
            SelectionItem::Point(expected_point),
            SelectionItem::Curve(expected_curve)
        ]
    );
    let after = local.state();
    assert_eq!(after.viewport, before.viewport);
    assert_eq!(after.visibility, before.visibility);
    assert_eq!(after.grid_visible, before.grid_visible);
    assert_eq!(
        after.curve_picks,
        vec![CurvePickContext {
            span: expected_curve,
            parameter: 0.5,
            origin: geosolve_constraint_editor::SceneCurveOrigin::Native
        }]
    );
    assert_eq!(local.dimensions.state.pins, vec![expected_dimension]);
    assert_eq!(local.dimensions.state.focus, Some(expected_dimension));
    assert_eq!(
        original,
        (
            first.export_project_json().unwrap(),
            first.workspace_design_json().unwrap(),
            second.export_project_json().unwrap(),
            second.workspace_design_json().unwrap()
        )
    );
}

#[test]
fn replacement_refuses_malformed_namespace_without_changing_personal_state() {
    let (_, _, mut local, mut seed) = fixture();
    select_and_pin(&mut local);
    let before = local.state_json().unwrap();
    seed["bindings"]["document"] =
        serde_json::to_value(local.scene.presentation_document().id()).unwrap();
    assert!(
        local
            .replace_json(&serde_json::json!({"seed":seed,"preserveSelection":true}).to_string())
            .is_err()
    );
    assert_eq!(local.state_json().unwrap(), before);
}

#[test]
fn replacement_drops_removed_incompatible_or_unavailable_owners_without_reusing_native_ids() {
    use geosolve_constraint_editor::IntentNativeBinding as Binding;
    for mode in ["removed", "incompatible", "unavailable"] {
        let (_, _, mut local, mut seed) = fixture();
        select_and_pin(&mut local);
        let curve = local.scene.curves[0].span.curve;
        let source = local.dimensions.state.focus.unwrap().source;
        let selected_symbols = local
            .bindings
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .filter(|(_, owned)| {
                owned.contains(&Binding::Curve(curve)) || owned.contains(&Binding::Source(source))
            })
            .map(|(symbol, _)| symbol.to_string())
            .collect::<Vec<_>>();
        assert!(!selected_symbols.is_empty());
        let nodes = seed["bindings"]["nodes"].as_object_mut().unwrap();
        for symbol in selected_symbols {
            if mode == "incompatible" {
                nodes
                    .get_mut(&symbol)
                    .unwrap()
                    .as_array_mut()
                    .unwrap()
                    .pop();
            } else {
                nodes.remove(&symbol);
            }
        }
        if mode == "unavailable" {
            seed.as_object_mut().unwrap().remove("bindings");
        }
        let before = local.state();
        local
            .replace_json(&serde_json::json!({"seed":seed,"preserveSelection":true}).to_string())
            .unwrap();
        assert!(local.editor.selection().is_empty(), "{mode}");
        assert!(local.dimensions.state.pins.is_empty(), "{mode}");
        assert!(local.dimensions.state.focus.is_none(), "{mode}");
        assert_eq!(local.state().viewport, before.viewport);
    }
}

#[test]
fn replacement_omits_ambiguous_point_correspondence_but_preserves_independent_curve_and_dimension()
{
    use geosolve_constraint_editor::IntentNativeBinding as Binding;
    let (mut first, _, mut local, mut seed) = fixture();
    let curve = local.scene.curves[0].span.curve;
    let (symbol, owned) = local
        .bindings
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .find(|(_, owned)| owned.contains(&Binding::Curve(curve)))
        .unwrap();
    let symbol = symbol.to_string();
    let owned = owned.clone();
    let alias: geosolve_sketch_intent::IntentKey =
        serde_json::from_value(serde_json::json!("replacement.ambiguous")).unwrap();
    let mut first_pair: serde_json::Value =
        serde_json::from_str(&first.interaction_snapshot_json().unwrap()).unwrap();
    first_pair["seed"]["bindings"]["nodes"][alias.as_str()] = serde_json::to_value(owned).unwrap();
    local = LocalInteraction::new(&first_pair["seed"].to_string()).unwrap();
    select_and_pin(&mut local);
    let mut swapped = seed["bindings"]["nodes"][&symbol].clone();
    let rows = swapped.as_array_mut().unwrap();
    assert_eq!(rows[0]["kind"], "point");
    assert_eq!(rows[1]["kind"], "point");
    rows.swap(0, 1);
    seed["bindings"]["nodes"]["replacement.ambiguous"] = swapped;
    let next = LocalInteraction::new(&seed.to_string()).unwrap();
    let expected = next.scene.curves[0].span;
    local
        .replace_json(&serde_json::json!({"seed":seed,"preserveSelection":true}).to_string())
        .unwrap();
    assert_eq!(local.editor.selection(), &[SelectionItem::Curve(expected)]);
    assert_eq!(local.state().curve_picks[0].span, expected);
    assert_eq!(local.dimensions.state.pins.len(), 1);
    assert!(local.dimensions.state.focus.is_some());
}

#[test]
fn replacement_preserves_only_the_exact_implicit_curve_occurrence() {
    let (_, _, mut local, seed) = fixture_from(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../geosolve-sketch-engine/tests/fixtures/point-gesture-computed.json"
    )));
    let discarded = local
        .scene
        .curves
        .iter()
        .find(|curve| curve.origin.is_implicit_construction())
        .unwrap();
    let pick = CurvePickContext {
        span: discarded.span,
        parameter: (discarded.screen_parameters.first().unwrap()
            + discarded.screen_parameters.last().unwrap())
            * 0.5,
        origin: discarded.origin,
    };
    set_selection(
        &mut local,
        SelectionPresentationState {
            items: vec![SelectionItem::Curve(pick.span)],
            curve_picks: vec![pick],
        },
    );
    let next = LocalInteraction::new(&seed.to_string()).unwrap();
    let expected = next
        .scene
        .curves
        .iter()
        .find(|curve| curve.origin.is_implicit_construction())
        .unwrap()
        .clone();
    assert_ne!(expected.origin, pick.origin);
    local
        .replace_json(&serde_json::json!({"seed":seed,"preserveSelection":true}).to_string())
        .unwrap();
    let actual = local.editor.selection_presentation_state();
    assert_eq!(
        actual.curve_picks,
        vec![CurvePickContext {
            span: expected.span,
            parameter: pick.parameter,
            origin: expected.origin
        }]
    );
    actual.validate(&local.scene).unwrap();
}
