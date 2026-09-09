// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch_code::{GeneratedSketchArtifact, GeneratedValue, bundled_sample};
use geosolve_sketch_engine::{EngineAuthoringMode, EngineComputedGeometry, SketchEngine};
use serde_json::{Value, json};

fn number(value: f64) -> Value {
    json!({ "kind": "number", "value": value })
}
fn point(x: f64, y: f64) -> Value {
    json!({ "kind": "array", "value": [number(x), number(y)] })
}
fn reference(identity: &str, path: &[&str], kind: &str) -> Value {
    json!({ "kind": "reference", "value": { "identity": [identity], "path": path, "kind": kind } })
}
fn object(fields: &Value) -> Value {
    json!({ "kind": "object", "value": fields })
}
fn circles(count: usize) -> Value {
    let declarations = (0..count).map(|index| json!({
        "identity": [format!("hole-{index}")], "family": "geometry.centerRadiusCircle",
        "arguments": object(&json!({"center": point(f64::from(u32::try_from(index).unwrap()) * 10.0, 0.0),
            "radius": { "kind": "unit", "value": { "unit": "mm", "value": 2.0 } }})),
    })).collect::<Vec<_>>();
    json!({ "format": "geosolve-generated-sketch-v1", "sdk_abi": "geosolve-sketch-code-v2",
        "declarations": declarations, "applications": [], "parameters": [], "groups": [],
        "suppressions": [], "document": { "title": "Loop generated holes" },
        "output": { "kind": "array", "value": (0..count).map(|index|
            reference(&format!("hole-{index}"), &["curve"], "curve")).collect::<Vec<_>>() },
    })
}

#[test]
fn generated_loops_have_finite_geometry_named_outputs_and_no_reverse_authority() {
    let mut engine = SketchEngine::new();
    let first = engine
        .evaluate_generated_json(&circles(3).to_string())
        .unwrap();
    let report = first.result();
    assert_eq!(report.mode, EngineAuthoringMode::Generator);
    assert!(!report.capabilities.managed_source_edits);
    assert!(!report.capabilities.reverse_geometry_edits);
    assert_eq!(report.geometry.curves.len(), 3);
    assert_eq!(report.named_outputs.len(), 3);
    assert!(report.validation.hard_residuals_validated);
    assert!(
        report
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1e-9)
    );
    for (index, x) in [0.0, 10.0, 20.0].into_iter().enumerate() {
        let expected = format!("/{index}");
        assert!(report.named_outputs.contains_key(&expected));
        assert!(report.geometry.points.iter().any(|point| (point.position[0] - x).abs() < 1e-9 && point.position[1].abs() < 1e-9));
    }
    let first_profiles = first.export_profiles(0.02).unwrap();
    assert_eq!(first_profiles["regions"].as_array().unwrap().len(), 3);
    let selected = first.export_profiles_for_output(0.02, "/1").unwrap();
    let regions = selected["regions"].as_array().unwrap();
    assert_eq!(regions.len(), 1);
    assert!(regions[0]["outer"].as_array().unwrap().iter().all(|point| {
        (8.0..=12.0).contains(&point[0].as_f64().unwrap())
            && (-2.0..=2.0).contains(&point[1].as_f64().unwrap())
    }));
    assert!(first.export_profiles_for_output(0.02, "/missing").is_err());
    let second = engine
        .evaluate_generated_json(&circles(1).to_string())
        .unwrap();
    assert_eq!(second.result().geometry.curves.len(), 1);
    assert_eq!(first.export_profiles(0.02).unwrap(), first_profiles);
    assert_eq!(
        first.export_profiles_for_output(0.02, "/1").unwrap(),
        selected
    );
    assert_ne!(first.result().result_id, second.result().result_id);
}

#[test]
fn named_outer_boundary_exports_complete_region_with_holes() {
    let mut input = circles(2);
    input["declarations"][0]["arguments"]["value"]["radius"]["value"]["value"] = 12.0.into();
    input["declarations"][1]["arguments"]["value"]["center"] = point(0.0, 0.0);
    let accepted = SketchEngine::new()
        .evaluate_generated_json(&input.to_string())
        .unwrap();
    let selected = accepted.export_profiles_for_output(0.002, "/0").unwrap();
    let regions = selected["regions"].as_array().unwrap();
    assert_eq!(regions.len(), 1);
    let holes = regions[0]["holes"].as_array().unwrap();
    assert_eq!(holes.len(), 1);
    let polygon_area = |points: &Value| {
        let points = points.as_array().unwrap();
        points
            .iter()
            .zip(points.iter().cycle().skip(1))
            .take(points.len())
            .map(|(a, b)| {
                a[0].as_f64().unwrap() * b[1].as_f64().unwrap()
                    - a[1].as_f64().unwrap() * b[0].as_f64().unwrap()
            })
            .sum::<f64>()
            * 0.5
    };
    let outer = polygon_area(&regions[0]["outer"]);
    let hole = polygon_area(&holes[0]);
    assert!(outer > 0.0 && hole < 0.0);
    assert!((outer + hole - std::f64::consts::PI * (144.0 - 4.0)).abs() < 0.2);
    let inner = accepted.export_profiles_for_output(0.002, "/1").unwrap();
    assert_eq!(inner["regions"].as_array().unwrap().len(), 1);
    assert!(inner["regions"][0]["holes"].as_array().unwrap().is_empty());
}

#[test]
fn rejects_duplicate_forward_forged_wrong_kind_nonfinite_and_invalid_geometry() {
    let mut engine = SketchEngine::new();
    let accepted = engine
        .evaluate_generated_json(&circles(1).to_string())
        .unwrap();
    let before = serde_json::to_value(accepted.result()).unwrap();
    let mut duplicate = circles(1);
    let extra = duplicate["declarations"][0].clone();
    duplicate["declarations"]
        .as_array_mut()
        .unwrap()
        .push(extra);
    let mut dangling = circles(1);
    dangling["output"] = reference("absent", &["curve"], "curve");
    let mut wrong_kind = circles(1);
    wrong_kind["output"] = reference("hole-0", &["curve"], "point");
    let mut wrong_path = circles(1);
    wrong_path["output"] = reference("hole-0", &["fake"], "curve");
    let mut invalid_radius = circles(1);
    invalid_radius["declarations"][0]["arguments"]["value"]["radius"]["value"]["value"] =
        json!(-2.0);
    let mut forward = circles(2);
    forward["declarations"][0]["arguments"]["value"]["center"] =
        reference("hole-1", &["center"], "point");
    for invalid in [
        duplicate,
        dangling,
        wrong_kind,
        wrong_path,
        invalid_radius,
        forward,
    ] {
        assert!(
            engine
                .evaluate_generated_json(&invalid.to_string())
                .is_err(),
            "{invalid}"
        );
        assert_eq!(
            serde_json::to_value(engine.last_accepted().unwrap().result()).unwrap(),
            before
        );
    }
    let mut nonfinite: GeneratedSketchArtifact = serde_json::from_value(circles(1)).unwrap();
    nonfinite.output = GeneratedValue::Number(f64::NAN);
    assert!(nonfinite.validate().is_err());
    assert!(
        engine
            .evaluate_generated_json("{\"format\":\"forged\"}")
            .is_err()
    );
}

#[test]
fn recorded_patch_channel_uses_shared_fillet_offset_and_cap_materialization() {
    let mut engine = SketchEngine::new();
    let accepted = engine
        .evaluate_generated_json(include_str!("fixtures/channel.json"))
        .unwrap();
    assert_eq!(
        accepted.result().document.title.as_deref(),
        Some("Recorded channel")
    );
    assert!(accepted.result().validation.all_active_features_current);
    assert_eq!(accepted.result().validation.feature_count, 2);
    let mut radii = accepted
        .result()
        .geometry
        .computed_edges
        .iter()
        .filter_map(|edge| match edge {
            EngineComputedGeometry::CircularArc { radius, .. } => Some(*radius),
            EngineComputedGeometry::NativeFragment { .. } => None,
        })
        .collect::<Vec<_>>();
    radii.sort_by(f64::total_cmp);
    assert_eq!(radii.len(), 2);
    for (actual, expected) in radii.iter().zip([6.0, 18.0]) {
        assert!((actual - expected).abs() < 1e-7);
    }
    assert!(!accepted.result().capabilities.reverse_geometry_edits);
    // This fixture leaves its start open inside the plate. Accepted geometry is
    // inspectable, but an incomplete profile must not be exported as a solid region.
    assert!(accepted.export_profiles(0.02).is_err());
}

#[test]
fn recorded_manifold_matches_managed_native_geometry_and_computed_radii() {
    let mut engine = SketchEngine::new();
    let generated = engine
        .evaluate_generated_json(include_str!("fixtures/manifold.json"))
        .unwrap();
    let managed = bundled_sample("pc-water-manifold").unwrap().project();
    let managed = engine
        .evaluate_managed_json(&managed.to_canonical_json().unwrap())
        .unwrap();
    let generated = generated.result();
    let managed = managed.result();
    assert!(generated.validation.hard_residuals_validated);
    assert!(generated.validation.all_active_features_current);
    assert_eq!(
        generated.validation.point_count,
        managed.validation.point_count
    );
    assert_eq!(
        generated.validation.curve_count,
        managed.validation.curve_count
    );
    assert_eq!(
        generated.validation.feature_count,
        managed.validation.feature_count
    );
    let positions = |result: &geosolve_sketch_engine::EngineAcceptedResult| {
        let mut positions = result
            .geometry
            .points
            .iter()
            .map(|point| point.position)
            .collect::<Vec<_>>();
        positions.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
        positions
    };
    let left = positions(generated);
    let right = positions(managed);
    for (left, right) in left.iter().zip(right) {
        assert!(
            (left[0] - right[0]).abs() < 1e-6 && (left[1] - right[1]).abs() < 1e-6,
            "{left:?} vs {right:?}"
        );
    }
}

#[test]
fn metadata_scopes_units_and_suppression_are_independently_admitted() {
    let mut input = circles(2);
    input["groups"] =
        json!([{ "name": "All mounting holes", "declarations": [["hole-0"], ["hole-1"]] }]);
    input["suppressions"] = json!([["hole-1"]]);
    input["parameters"] = json!([{ "id": "hole-radius", "value": { "kind": "unit", "value": { "unit": "cm", "value": 0.2 } },
        "presentation": { "label": "Hole radius", "description": "Shared design input", "isKeyParameter": true } }]);
    input["declarations"][0]["arguments"]["value"]["radius"] =
        json!({ "kind": "unit", "value": { "unit": "cm", "value": 0.2 } });
    let mut engine = SketchEngine::new();
    let result = engine.evaluate_generated_json(&input.to_string()).unwrap();
    assert_eq!(
        result
            .result()
            .geometry
            .curves
            .iter()
            .filter(|curve| !curve.visible_intervals.is_empty())
            .count(),
        1
    );
    assert_eq!(
        result
            .result()
            .generated_metadata
            .as_ref()
            .unwrap()
            .parameters[0]
            .presentation
            .is_key_parameter,
        Some(true)
    );
    let polygons = result.export_profiles(0.02).unwrap();
    assert_eq!(polygons["regions"].as_array().unwrap().len(), 1);
    for point in polygons["regions"][0]["outer"].as_array().unwrap() {
        assert!((point[0].as_f64().unwrap().hypot(point[1].as_f64().unwrap()) - 2.0).abs() < 1e-9);
    }
    let mut bad = input.clone();
    bad["declarations"][0]["arguments"]["value"]["isKeyConstraint"] =
        json!({ "kind": "bool", "value": true });
    assert!(engine.evaluate_generated_json(&bad.to_string()).is_err());
    let mut bad = input.clone();
    bad["parameters"][0]["presentation"]["isKeyParameter"] = json!("yes");
    assert!(engine.evaluate_generated_json(&bad.to_string()).is_err());
    let mut bad = input;
    bad["declarations"][0]["arguments"]["value"]["radius"] =
        json!({ "kind": "unit", "value": { "unit": "m", "value": 1.7e308 } });
    assert!(engine.evaluate_generated_json(&bad.to_string()).is_err());
}
