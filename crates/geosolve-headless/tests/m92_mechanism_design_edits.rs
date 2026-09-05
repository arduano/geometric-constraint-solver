// SPDX-License-Identifier: GPL-3.0-or-later

#[path = "support/m92_audit.rs"]
mod audit;

use geosolve_headless::HeadlessInput;
use geosolve_sketch_code::bundled_sample;

fn named_points(
    project: &geosolve_sketch_code::CodeProject,
) -> std::collections::BTreeMap<String, [f64; 2]> {
    use geosolve_sketch::{DocumentId, PersistentId};
    use geosolve_sketch_code::{CodeOwnerAddress, CodePointEdit, materialize_code_project_cold};
    let materialized = materialize_code_project_cold(
        project,
        &audit::generated(project),
        geosolve_sketch_intent::IntentSessionId::from_raw(0x9294),
        DocumentId(PersistentId::from_u128(0x9294)),
        1.0,
    )
    .unwrap();
    let coordinator = materialized.editor.coordinator();
    let accepted = coordinator.accepted_materialization().unwrap();
    let document = accepted
        .session
        .accepted_state_for_current_input()
        .unwrap()
        .document();
    materialized
        .expansion
        .writable_points
        .iter()
        .filter_map(|point| {
            let CodePointEdit::Point { address } = &point.edit else {
                return None;
            };
            let CodeOwnerAddress::DirectDeclaration { declaration } = &address.owner.address else {
                return None;
            };
            let node = coordinator
                .intent()
                .graph()
                .node_by_symbol(&point.handle.alias)
                .unwrap();
            let port = node.port_by_selector(point.handle.selector).unwrap();
            let geosolve_constraint_editor::IntentNativeBinding::Point(id) =
                accepted.ownership.port(port.as_ref(node.id)).unwrap()
            else {
                return None;
            };
            Some((declaration.0.clone(), document.point(id).unwrap().position))
        })
        .collect()
}
fn point(points: &std::collections::BTreeMap<String, [f64; 2]>, declaration: &str) -> [f64; 2] {
    *points
        .get(declaration)
        .unwrap_or_else(|| panic!("missing {declaration}"))
}
fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

#[test]
#[allow(
    clippy::too_many_lines,
    clippy::float_cmp,
    reason = "the complete ten-edit matrix asserts exact fixed datums alongside independently measured relationships"
)]
fn every_mechanism_has_two_measured_reversible_design_edits() {
    let cases = [
        (
            "theo-jansen-leg",
            "crankLength",
            16.0,
            "upperRockerLength",
            42.0,
        ),
        (
            "whitworth-quick-return",
            "returnLinkLength",
            8.5,
            "crankLength",
            3.0,
        ),
        (
            "twin-roller-bezier-cam",
            "dimension1CamRollerRadius1",
            1.2,
            "camRise",
            5.0,
        ),
        (
            "peaucellier-linkage",
            "dimension2PeaucellierRhombusSide3",
            3.2,
            "dimension1PeaucellierLongRadius5",
            5.2,
        ),
        (
            "five-stage-scissor-lift",
            "dimension1TowerMasterDiagonalLength10",
            10.5,
            "guideLength",
            9.5,
        ),
    ];
    for (key, first, first_value, second, second_value) in cases {
        if let Some(root) = std::env::var_os("M92_AUDIT_OUTPUT") {
            std::fs::create_dir_all(std::path::PathBuf::from(root).join(key)).unwrap();
        }
        let base = bundled_sample(key).unwrap().project();
        let original = named_points(&base);
        for (index, (declaration, value)) in [(first, first_value), (second, second_value)]
            .into_iter()
            .enumerate()
        {
            let edited = audit::edit_mm(
                &HeadlessInput::BundledSample(key.into()),
                declaration,
                if declaration == "camRise" {
                    &["target"]
                } else {
                    &["value"]
                },
                value,
            );
            audit::assert_valid(&edited);
            audit::assert_history(&base, edited.project());
            let doc = audit::accepted_document(edited.project());
            let points = named_points(edited.project());
            match key {
                "theo-jansen-leg" => {
                    let (a, b) = if index == 0 {
                        ("groundPivot", "crankPin")
                    } else {
                        ("framePivot", "upperKnee")
                    };
                    assert!((distance(point(&points, a), point(&points, b)) - value).abs() < 1e-7);
                    assert_eq!(
                        point(&points, "groundPivot"),
                        point(&original, "groundPivot")
                    );
                    assert_eq!(point(&points, "framePivot"), point(&original, "framePivot"));
                    assert!(distance(point(&points, "foot"), point(&original, "foot")) > 0.01);
                }
                "whitworth-quick-return" => {
                    let (a, b) = if index == 0 {
                        ("rockerEnd", "ramPin")
                    } else {
                        ("crankPivot", "crankPin")
                    };
                    assert!((distance(point(&points, a), point(&points, b)) - value).abs() < 1e-7);
                    assert!((point(&points, "ramPin")[1] - 6.0).abs() < 1e-8);
                    assert_eq!(point(&points, "crankPivot"), [0.0, 0.0]);
                    assert!(distance(point(&points, "rockerPivot"), [0.0, -5.0]) < 1e-9);
                }
                "twin-roller-bezier-cam" => {
                    let radii = doc
                        .scalars()
                        .iter()
                        .filter(|s| matches!(s.unit, geosolve_sketch::ScalarUnit::Length))
                        .map(|s| s.value)
                        .collect::<Vec<_>>();
                    assert_eq!(radii.len(), 2);
                    assert!(
                        radii
                            .into_iter()
                            .all(|r| (r - if index == 0 { value } else { 1.0 }).abs() < 1e-8)
                    );
                    for label in ["point1CamQ0", "point3CamQ2"] {
                        assert_eq!(point(&points, label), point(&original, label));
                    }
                    assert!(
                        distance(
                            point(&points, "point2CamQ1"),
                            [0.0, if index == 0 { 4.0 } else { value }]
                        ) < 1e-9
                    );
                }
                "peaucellier-linkage" => {
                    let (long, side) = if index == 0 {
                        (5.0, value)
                    } else {
                        (value, 3.0)
                    };
                    let output = point(&points, "point4PeaucellierStraightLineOutputQ");
                    assert!((output[0] - (long * long - side * side) / 8.0).abs() < 1e-7);
                    assert_eq!(point(&points, "point1PeaucellierFixedOriginO"), [0.0, 0.0]);
                    assert_eq!(point(&points, "point2PeaucellierInputCenterS"), [4.0, 0.0]);
                }
                "five-stage-scissor-lift" => {
                    for stage in 0..5 {
                        let l = point(
                            &points,
                            &format!("point{}TowerLevel{stage}Left", stage * 2 + 1),
                        );
                        let r = point(
                            &points,
                            &format!("point{}TowerLevel{}Right", stage * 2 + 4, stage + 1),
                        );
                        assert!(
                            (distance(l, r) - if index == 0 { value } else { 10.0 }).abs() < 1e-7
                        );
                    }
                    assert_eq!(point(&points, "point1TowerLevel0Left"), [-4.0, 0.0]);
                    assert!(
                        (point(&points, "guideEnd")[0]
                            - if index == 0 { 6.0 } else { value - 4.0 })
                        .abs()
                            < 1e-7
                    );
                }
                _ => unreachable!(),
            }
            audit::preserve(&edited, key, &format!("edit-{}", index + 1));
        }
    }
}
