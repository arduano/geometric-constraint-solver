// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch::{
    CurveDefinition, ScalarDomain, ScalarUnit, SketchDocument, VisualProfileOptions,
    VisualProfileStatus,
};

fn disjoint_channel_and_bore(scale: f64, reflected: bool) -> SketchDocument {
    let mut document = SketchDocument::new(scale).unwrap();
    let map = |point: [f64; 2]| {
        [
            scale * point[0],
            scale * point[1] * if reflected { -1.0 } else { 1.0 },
        ]
    };
    let vertices: [[f64; 2]; 8] = [
        [-25.0, -5.0],
        [-5.0, -5.0],
        [-5.0, -25.0],
        [25.0, -25.0],
        [25.0, -15.0],
        [5.0, -15.0],
        [5.0, 5.0],
        [-25.0, 5.0],
    ];
    let vertices = vertices.map(map);
    let points = vertices
        .iter()
        .map(|position| document.add_point("channel vertex", *position).unwrap())
        .collect();
    let directions = vertices
        .iter()
        .zip(vertices.iter().cycle().skip(1))
        .take(vertices.len())
        .map(|(a, b)| {
            let delta = [b[0] - a[0], b[1] - a[1]];
            let length = delta[0].hypot(delta[1]);
            [delta[0] / length, delta[1] / length]
        })
        .collect();
    document
        .add_curve(
            "channel",
            CurveDefinition::Polyline {
                points,
                closed: true,
                branch_directions: directions,
            },
        )
        .unwrap();
    let center = document.add_point("bore center", map([20.0, 2.0])).unwrap();
    let radius = document
        .add_scalar(
            "bore radius",
            3.0 * scale,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    document
        .add_curve("bore", CurveDefinition::Circle { center, radius })
        .unwrap();
    document
}

#[test]
fn m98_f007_disjoint_contours_do_not_touch_when_containment_ray_is_tangent() {
    for (scale, reflected) in [(1.0, false), (1.0, true), (0.01, false), (10.0, true)] {
        let document = disjoint_channel_and_bore(scale, reflected);
        let center = document.points()[8].position;
        let vertices = &document.points()[..8];
        for (a, b) in vertices.iter().zip(vertices.iter().cycle().skip(1)) {
            let delta = [b.position[0] - a.position[0], b.position[1] - a.position[1]];
            let t = (((center[0] - a.position[0]) * delta[0]
                + (center[1] - a.position[1]) * delta[1])
                / (delta[0] * delta[0] + delta[1] * delta[1]))
                .clamp(0.0, 1.0);
            let distance = (center[0] - a.position[0] - t * delta[0])
                .hypot(center[1] - a.position[1] - t * delta[1]);
            assert!(distance - 3.0 * scale >= 11.9 * scale);
        }
        let before = document.to_canonical_json().unwrap();
        let analysis = document.analyze_visual_profiles(VisualProfileOptions::default());
        assert_eq!(
            analysis.status,
            VisualProfileStatus::Complete,
            "{analysis:#?}"
        );
        assert!(analysis.issues.is_empty(), "{analysis:#?}");
        assert_eq!(analysis.faces.len(), 2);
        assert!(analysis.faces.iter().all(|face| face.contours.len() == 1
            && face.visual_area.is_finite()
            && face.visual_area > 0.0));
        let total = analysis
            .faces
            .iter()
            .map(|face| face.visual_area)
            .sum::<f64>();
        assert!(
            (total - (700.0 + 9.0 * std::f64::consts::PI) * scale * scale).abs()
                < 1e-9 * scale * scale
        );
        assert_eq!(document.to_canonical_json().unwrap(), before);
    }
}
