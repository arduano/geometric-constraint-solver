// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch_engine::{AcceptedEvaluation, SketchEngine};
use serde::Deserialize;
use serde_json::{Value, json};

fn reference(identity: &[&str], path: &[&str], kind: &str) -> Value {
    json!({"kind":"reference","value":{"identity":identity,"path":path,"kind":kind}})
}

fn channel(width: f64) -> Value {
    let mut artifact: Value = serde_json::from_str(include_str!("fixtures/channel.json")).unwrap();
    artifact["declarations"]
        .as_array_mut()
        .unwrap()
        .retain(|declaration| {
            declaration["identity"][0] != "plate" && declaration["identity"][0] != "port"
        });
    artifact["declarations"][1]["arguments"]["value"]["caps"]["value"] = "both".into();
    artifact["declarations"][1]["arguments"]["value"]["width"]["value"]["value"] = width.into();
    artifact["applications"][0]["inputs"]["value"]["width"]["value"]["value"] = width.into();
    artifact["parameters"][0]["value"]["value"]["value"] = width.into();
    artifact["groups"] = json!([]);
    artifact["output"] = json!({"kind":"object","value": {
        "channel": reference(&["passage","channel"], &[], "feature"),
        "centerline": reference(&["centerline"], &[], "feature"),
    }});
    artifact
}

fn evaluate(artifact: &Value) -> AcceptedEvaluation {
    SketchEngine::new()
        .evaluate_generated_json(&artifact.to_string())
        .unwrap()
}

#[derive(Debug, Deserialize)]
struct Region {
    outer: Vec<[f64; 2]>,
    holes: Vec<Vec<[f64; 2]>>,
}

fn regions(value: &Value) -> Vec<Region> {
    serde_json::from_value(value["regions"].clone()).unwrap()
}

fn area(points: &[[f64; 2]]) -> f64 {
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .map(|(a, b)| a[0] * b[1] - a[1] * b[0])
        .sum::<f64>()
        * 0.5
}

fn channel_area(width: f64, scale: f64) -> f64 {
    let centerline_length = (80.0 - 24.0 + 12.0 * std::f64::consts::FRAC_PI_2) * scale;
    centerline_length * width + std::f64::consts::PI * (width * 0.5).powi(2)
}

fn channel_boundary_distance(point: [f64; 2]) -> f64 {
    // Independent analytic outline of the 12 mm fixture: four straight walls,
    // concentric quarter circles at its bend, and two semicircular end caps.
    let segments: [([f64; 2], [f64; 2]); 4] = [
        ([0.0, -6.0], [28.0, -6.0]),
        ([46.0, 12.0], [46.0, 40.0]),
        ([34.0, 12.0], [34.0, 40.0]),
        ([0.0, 6.0], [28.0, 6.0]),
    ];
    let mut distance = f64::INFINITY;
    for (a, b) in segments {
        let delta = [b[0] - a[0], b[1] - a[1]];
        let parameter = (((point[0] - a[0]) * delta[0] + (point[1] - a[1]) * delta[1])
            / (delta[0] * delta[0] + delta[1] * delta[1]))
            .clamp(0.0, 1.0);
        distance = distance.min(
            (point[0] - a[0] - parameter * delta[0]).hypot(point[1] - a[1] - parameter * delta[1]),
        );
    }
    let pi = std::f64::consts::PI;
    for (center, radius, start, end) in [
        ([28.0, 12.0], 6.0, 1.5 * pi, 2.0 * pi),
        ([28.0, 12.0], 18.0, 1.5 * pi, 2.0 * pi),
        ([0.0, 0.0], 6.0, 0.5 * pi, 1.5 * pi),
        ([40.0, 40.0], 6.0, 0.0, pi),
    ] {
        let offset = [point[0] - center[0], point[1] - center[1]];
        let angle = offset[1].atan2(offset[0]).rem_euclid(2.0 * pi);
        if (start..=end).contains(&angle) {
            distance = distance.min((offset[0].hypot(offset[1]) - radius).abs());
        }
        for angle in [start, end] {
            distance = distance
                .min((offset[0] - radius * angle.cos()).hypot(offset[1] - radius * angle.sin()));
        }
    }
    distance
}

#[test]
fn closed_channel_profiles_have_analytic_area_and_preserve_accepted_geometry() {
    let accepted = evaluate(&channel(12.0));
    assert!(accepted.result().capabilities.profile_export);
    let before = serde_json::to_value(accepted.result()).unwrap();
    let exported = accepted.export_profiles(0.002).unwrap();
    let polygons = regions(&exported);
    assert_eq!(polygons.len(), 1);
    assert!(polygons[0].holes.is_empty());
    let expected = channel_area(12.0, 1.0);
    assert!(
        (area(&polygons[0].outer) - expected).abs() < 0.1,
        "{} vs {expected}",
        area(&polygons[0].outer)
    );
    assert_eq!(serde_json::to_value(accepted.result()).unwrap(), before);
    assert!(
        polygons[0]
            .outer
            .iter()
            .flatten()
            .all(|value| value.is_finite())
    );
    for (a, b) in polygons[0]
        .outer
        .iter()
        .zip(polygons[0].outer.iter().cycle().skip(1))
    {
        assert!(channel_boundary_distance(*a) < 1e-10);
        let midpoint = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
        assert!(
            channel_boundary_distance(midpoint) <= 0.002 + 1e-10,
            "chord midpoint {midpoint:?}"
        );
    }
    assert_eq!(
        accepted
            .export_profiles_for_output(0.002, "/channel")
            .unwrap(),
        exported
    );
    assert!(
        accepted
            .export_profiles_for_output(0.002, "/centerline")
            .is_err()
    );
    assert!(
        accepted
            .export_profiles_for_output(0.002, "/missing")
            .is_err()
    );
    for chord in [f64::NAN, f64::INFINITY, -1.0, 0.0, 1.0e-30] {
        assert!(accepted.export_profiles(chord).is_err(), "chord {chord}");
    }
    assert_eq!(serde_json::to_value(accepted.result()).unwrap(), before);
}

fn transformed_channel(width: f64, scale: f64, reflected: bool) -> Value {
    let mut artifact = channel(width * scale);
    let vertices = artifact["declarations"][0]["arguments"]["value"]["vertices"]["value"]
        .as_array_mut()
        .unwrap();
    for vertex in vertices {
        let coordinates = vertex["value"]["position"]["value"].as_array_mut().unwrap();
        for (axis, coordinate) in coordinates.iter_mut().enumerate() {
            coordinate["value"] = (coordinate["value"].as_f64().unwrap()
                * scale
                * if reflected && axis == 1 { -1.0 } else { 1.0 })
            .into();
        }
    }
    artifact["declarations"][1]["arguments"]["value"]["bendRadius"]["value"]["value"] =
        (12.0 * scale).into();
    artifact
}

#[test]
fn channel_width_reflection_scale_and_chord_refinement_preserve_shape() {
    for (width, scale, reflected) in [(8.0, 1.0, false), (12.0, 0.01, true), (12.0, 10.0, true)] {
        let artifact = transformed_channel(width, scale, reflected);
        let accepted = evaluate(&artifact);
        let fine = regions(
            &accepted
                .export_profiles(0.002 * scale)
                .unwrap_or_else(|error| {
                    panic!("width={width} scale={scale} reflected={reflected}: {error}")
                }),
        );
        let coarse = regions(&accepted.export_profiles(0.1 * scale).unwrap());
        let expected = channel_area(width * scale, scale);
        assert_eq!(fine.len(), 1);
        assert_eq!(coarse.len(), 1);
        assert!((area(&fine[0].outer) - expected).abs() < 0.1 * scale * scale);
        assert!(
            (area(&fine[0].outer) - expected).abs() < (area(&coarse[0].outer) - expected).abs()
        );
        assert!(fine[0].outer.len() > coarse[0].outer.len());
    }
}

#[test]
fn oversized_geometry_retains_native_area_certification_failure() {
    // The engine's explicit 1 mm model scale asks the owning topology oracle for
    // 1e-9 mm2 area certainty; this 4 m route cannot certify that bound.
    let accepted = evaluate(&transformed_channel(12.0, 100.0, true));
    let before = serde_json::to_value(accepted.result()).unwrap();
    assert!(accepted.result().validation.hard_residuals_validated);
    let failure = accepted.export_profiles(0.2).unwrap_err().to_string();
    assert!(
        failure.contains("incomplete production topology"),
        "{failure}"
    );
    assert!(failure.contains("SourceEvaluationFailed"), "{failure}");
    assert_eq!(serde_json::to_value(accepted.result()).unwrap(), before);
}

#[test]
fn selected_computed_channel_preserves_native_bore_hole() {
    let mut artifact = channel(12.0);
    let fixture: Value = serde_json::from_str(include_str!("fixtures/channel.json")).unwrap();
    let mut circle = fixture["declarations"][3].clone();
    circle["arguments"]["value"]["center"]["value"][0]["value"] = 20.0.into();
    circle["arguments"]["value"]["center"]["value"][1]["value"] = 0.0.into();
    circle["arguments"]["value"]["radius"]["value"]["value"] = 2.0.into();
    artifact["declarations"]
        .as_array_mut()
        .unwrap()
        .push(circle);
    artifact["output"]["value"]["bore"] = reference(&["port"], &["curve"], "curve");
    let accepted = evaluate(&artifact);
    let selected = regions(
        &accepted
            .export_profiles_for_output(0.002, "/channel")
            .unwrap(),
    );
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].holes.len(), 1);
    assert!(area(&selected[0].holes[0]) < 0.0);
    let net = area(&selected[0].outer) + area(&selected[0].holes[0]);
    assert!((net - (channel_area(12.0, 1.0) - std::f64::consts::PI * 4.0)).abs() < 0.1);
    let bore = regions(&accepted.export_profiles_for_output(0.002, "/bore").unwrap());
    assert_eq!(bore.len(), 1);
    assert!(bore[0].holes.is_empty());
    assert!((area(&bore[0].outer) - std::f64::consts::PI * 4.0).abs() < 0.02);
    assert_eq!(regions(&accepted.export_profiles(0.002).unwrap()).len(), 2);
}

#[test]
fn open_channel_fails_completely_without_replacing_accepted_result() {
    let mut artifact = channel(12.0);
    artifact["declarations"][1]["arguments"]["value"]["caps"]["value"] = "end".into();
    let mut engine = SketchEngine::new();
    let accepted = engine
        .evaluate_generated_json(&artifact.to_string())
        .unwrap();
    let before = serde_json::to_value(accepted.result()).unwrap();
    let failure = accepted.export_profiles(0.02).unwrap_err().to_string();
    assert!(
        failure.contains("incomplete production topology"),
        "{failure}"
    );
    let closed = engine
        .evaluate_generated_json(&channel(12.0).to_string())
        .unwrap();
    assert_eq!(regions(&closed.export_profiles(0.02).unwrap()).len(), 1);
    assert_eq!(serde_json::to_value(accepted.result()).unwrap(), before);
    assert!(accepted.export_profiles(0.02).is_err());
}

#[test]
fn complete_manifold_exports_all_channels_seal_ports_and_plate_faces() {
    let artifact: Value = serde_json::from_str(include_str!("fixtures/manifold.json")).unwrap();
    let accepted = evaluate(&artifact);
    let polygons = regions(&accepted.export_profiles(0.02).unwrap());
    // One plate, eight fastener disks, the seal and its interior, the connected
    // wet circuit with three port disks, and the stair passage with two disks.
    assert_eq!(polygons.len(), 18, "complete bounded face inventory");
    let outermost = polygons
        .iter()
        .max_by(|a, b| area(&a.outer).total_cmp(&area(&b.outer)))
        .unwrap();
    assert!((area(&outermost.outer) - 240.0 * 120.0).abs() < 1.0e-6);
    assert_eq!(outermost.holes.len(), 9);
    assert!(polygons.iter().all(|region| area(&region.outer) > 0.0));
    assert!(
        polygons
            .iter()
            .flat_map(|region| &region.holes)
            .all(|hole| area(hole) < 0.0)
    );
    assert!(polygons.iter().all(|region| {
        region
            .outer
            .iter()
            .chain(region.holes.iter().flatten())
            .flatten()
            .all(|value| value.is_finite())
    }));
    let total_area: f64 = polygons
        .iter()
        .map(|region| area(&region.outer) + region.holes.iter().map(|hole| area(hole)).sum::<f64>())
        .sum();
    assert!(
        (total_area - 240.0 * 120.0).abs() < 1.0e-6,
        "all bounded faces partition the plate"
    );
    let disks = polygons
        .iter()
        .filter(|region| region.holes.is_empty())
        .collect::<Vec<_>>();
    assert_eq!(disks.len(), 13);
    assert_eq!(
        disks
            .iter()
            .filter(|region| (area(&region.outer) - std::f64::consts::PI * 9.0).abs() < 0.3)
            .count(),
        5
    );
    assert_eq!(
        disks
            .iter()
            .filter(|region| (area(&region.outer) - std::f64::consts::PI * 6.25).abs() < 0.3)
            .count(),
        8
    );
}

#[test]
fn disjoint_channel_and_bore_export_when_a_containment_ray_aligns_with_a_wall() {
    for width in [10.0, 11.0, 12.0] {
        let mut artifact = channel(width);
        for (vertex, position) in
            artifact["declarations"][0]["arguments"]["value"]["vertices"]["value"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .zip([[-20.0, 0.0], [0.0, 0.0], [0.0, -20.0]])
        {
            vertex["value"]["position"]["value"][0]["value"] = position[0].into();
            vertex["value"]["position"]["value"][1]["value"] = position[1].into();
        }
        let mut last =
            artifact["declarations"][0]["arguments"]["value"]["vertices"]["value"][2].clone();
        last["value"]["key"]["value"] = "d".into();
        last["value"]["position"]["value"][0]["value"] = 20.0.into();
        artifact["declarations"][0]["arguments"]["value"]["vertices"]["value"]
            .as_array_mut()
            .unwrap()
            .push(last);
        artifact["declarations"][1]["arguments"]["value"]["bendRadius"]["value"]["value"] =
            8.0.into();
        artifact["applications"][0]["inputs"]["value"]["bendRadius"]["value"]["value"] = 8.0.into();
        let fixture: Value = serde_json::from_str(include_str!("fixtures/channel.json")).unwrap();
        let mut circle = fixture["declarations"][3].clone();
        circle["arguments"]["value"]["center"]["value"][0]["value"] = 20.0.into();
        circle["arguments"]["value"]["center"]["value"][1]["value"] = 2.0.into();
        circle["arguments"]["value"]["radius"]["value"]["value"] = 3.0.into();
        artifact["declarations"]
            .as_array_mut()
            .unwrap()
            .push(circle);
        let accepted = evaluate(&artifact);
        let before = serde_json::to_value(accepted.result()).unwrap();
        let polygons = regions(&accepted.export_profiles(0.002).unwrap());
        assert_eq!(polygons.len(), 2);
        assert!(
            polygons
                .iter()
                .all(|region| region.holes.is_empty() && area(&region.outer) > 0.0)
        );
        let expected = (28.0 + 8.0 * std::f64::consts::PI) * width
            + std::f64::consts::PI * (width * width * 0.25 + 9.0);
        let actual = polygons
            .iter()
            .map(|region| area(&region.outer))
            .sum::<f64>();
        assert!(
            (actual - expected).abs() < 0.2,
            "width {width}: {actual} vs {expected}"
        );
        assert_eq!(serde_json::to_value(accepted.result()).unwrap(), before);
    }
}
