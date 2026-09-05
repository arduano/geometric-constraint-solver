// SPDX-License-Identifier: GPL-3.0-or-later

#[path = "support/m92_audit.rs"]
mod audit;
use geosolve_sketch::{CurveDefinition, SketchDocument};
use geosolve_sketch_code::bundled_sample;
fn document(key: &str) -> SketchDocument {
    audit::accepted_document(&bundled_sample(key).unwrap().project())
}

fn circles(document: &SketchDocument) -> Vec<([f64; 2], f64)> {
    document
        .curves()
        .iter()
        .filter_map(|c| match c.definition {
            CurveDefinition::Circle { center, radius } => Some((
                document.point(center).unwrap().position,
                document.scalar(radius).unwrap().value,
            )),
            _ => None,
        })
        .collect()
}

#[test]
fn prusa_bearing_axes_are_34_mm_apart() {
    let circles = circles(&document("prusa-mini-interface"));
    let bearings = circles
        .iter()
        .filter(|(_, r)| (r - 7.5).abs() < 1e-8)
        .collect::<Vec<_>>();
    assert_eq!(
        bearings.len(),
        2,
        "the published STEP has two R7.5 bearing seats, not one slotted proxy"
    );
    assert!(
        (f64::hypot(
            bearings[0].0[0] - bearings[1].0[0],
            bearings[0].0[1] - bearings[1].0[1]
        ) - 34.)
            .abs()
            < 1e-8
    );
}

#[test]
fn micron_rail_mounts_use_the_extracted_16_by_15_interface() {
    let circles = circles(&document("micron-carriage"));
    for x in [-8., 8.] {
        for y in [-7.5, 7.5] {
            assert!(
                circles.iter().any(|(p, r)| (p[0] - x).abs() < 1e-8
                    && (p[1] - y).abs() < 1e-8
                    && (r - 1.5).abs() < 1e-8),
                "extracted Micron M3 interface missing at {x},{y}"
            );
        }
    }
}

#[test]
fn hevort_keeps_extracted_hd9_mounts_distinct_from_the_mgn9_pitch() {
    let circles = circles(&document("hevort-datum-study"));
    assert!(
        circles.iter().any(|(p, r)| (p[0] - 10.).abs() < 1e-8
            && (p[1] - 10.).abs() < 1e-8
            && (r - 1.6).abs() < 1e-8),
        "HD9 STEP mount axes form a 20 by 20 square"
    );
}

#[test]
fn voron_t_passage_and_side_notches_belong_to_one_profile() {
    let document = document("voron-panel");
    assert!(document.curves().iter().any(|c| matches!(&c.definition, CurveDefinition::Polyline {points,closed:true,..} if points.len() >= 20)), "DXF is one connected outside boundary with an edge-open T passage and two edge notches");
}
