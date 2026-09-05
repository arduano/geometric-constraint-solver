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
fn voron_t_passage_and_side_notches_belong_to_one_profile() {
    let document = document("voron-panel");
    assert!(document.curves().iter().any(|c| matches!(&c.definition, CurveDefinition::Polyline {points,closed:true,..} if points.len() >= 20)), "DXF is one connected outside boundary with an edge-open T passage and two edge notches");
}

#[test]
#[ignore = "requires the built pinned TypeScript mutation sidecar"]
fn bondtech_pitch_edit_moves_the_three_seats() {
    let input = geosolve_headless::HeadlessInput::BundledSample("bondtech-indx-link".into());
    let edited = audit::edit_mm(&input, "couplingPitchRadius", &["value"], 15.0);
    let document = audit::accepted_document(edited.project());
    let seats = circles(&document)
        .into_iter()
        .filter(|(_, r)| (r - 2.5).abs() < 1e-8)
        .collect::<Vec<_>>();
    assert_eq!(seats.len(), 3);
    for (point, _) in seats {
        assert!(
            (point[0].hypot(point[1]) - 15.0).abs() < 1e-8,
            "pitch edit left coupling seat behind at {point:?}"
        );
    }
}

fn near(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-8,
        "measured {actual}, expected {expected}"
    );
}
type NamedCircles = std::collections::BTreeMap<String, ([f64; 2], f64)>;
fn named_circle_geometry(project: &geosolve_sketch_code::CodeProject) -> NamedCircles {
    use geosolve_constraint_editor::IntentNativeBinding;
    use geosolve_sketch::{DocumentId, PersistentId};
    use geosolve_sketch_code::materialize_code_project_cold;
    let materialized = materialize_code_project_cold(
        project,
        &audit::generated(project),
        geosolve_sketch_intent::IntentSessionId::from_raw(0x92_b0),
        DocumentId(PersistentId::from_u128(0x92_b0)),
        1.0,
    )
    .unwrap();
    let coordinator = materialized.editor.coordinator();
    let accepted = coordinator.accepted_materialization().unwrap();
    let doc = accepted
        .session
        .accepted_state_for_current_input()
        .unwrap()
        .document();
    let mut circles = NamedCircles::new();
    for (alias, declaration) in &materialized.expansion.declaration_provenance {
        let node = coordinator.intent().graph().node_by_symbol(alias).unwrap();
        for port in node.ports.values() {
            let Some(IntentNativeBinding::Curve(id)) =
                accepted.ownership.port(port.as_ref(node.id))
            else {
                continue;
            };
            if let CurveDefinition::Circle { center, radius } = doc.curve(id).unwrap().definition {
                circles.insert(
                    declaration.0.clone(),
                    (
                        doc.point(center).unwrap().position,
                        doc.scalar(radius).unwrap().value,
                    ),
                );
            }
        }
    }
    circles
}
fn named_circles(circles: &NamedCircles, prefix: &str) -> Vec<([f64; 2], f64)> {
    circles
        .iter()
        .filter(|(name, _)| name.starts_with(prefix))
        .map(|(_, value)| *value)
        .collect()
}
fn square(doc: &NamedCircles, label: &str, center: [f64; 2], width: f64, height: f64, radius: f64) {
    let holes = named_circles(doc, label);
    assert_eq!(
        holes.len(),
        4,
        "{label} hole count; curve labels {:?}",
        doc.keys().collect::<Vec<_>>()
    );
    for (p, r) in holes {
        near((p[0] - center[0]).abs(), width / 2.);
        near((p[1] - center[1]).abs(), height / 2.);
        near(r, radius);
    }
}
fn pair(doc: &NamedCircles, label: &str, center: [f64; 2], pitch: f64, radius: f64, axis: usize) {
    let holes = named_circles(doc, label);
    assert_eq!(holes.len(), 2, "{label} pair");
    for (p, r) in holes {
        near((p[axis] - center[axis]).abs(), pitch / 2.);
        near(p[1 - axis], center[1 - axis]);
        near(r, radius);
    }
}
fn polygon(doc: &SketchDocument) -> Vec<[f64; 2]> {
    doc.curves()
        .iter()
        .find_map(|c| match &c.definition {
            CurveDefinition::Polyline {
                points,
                closed: true,
                ..
            } => Some(
                points
                    .iter()
                    .map(|p| doc.point(*p).unwrap().position)
                    .collect(),
            ),
            _ => None,
        })
        .expect("one closed authored outline")
}
fn contains(poly: &[[f64; 2]], p: [f64; 2]) -> bool {
    let mut inside = false;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
    }
    inside
}
#[allow(
    clippy::too_many_lines,
    reason = "keeps independently measured expectations for the three reference studies adjacent"
)]
fn intent(project: &geosolve_sketch_code::CodeProject, key: &str, edit: Option<usize>) {
    let native = audit::accepted_document(project);
    let doc = &named_circle_geometry(project);
    match key {
        "micron-carriage" => {
            square(
                doc,
                "railDatumHole",
                [0., 0.],
                if edit == Some(0) { 18. } else { 16. },
                15.,
                1.5,
            );
            square(doc, "alternateHole", [0., 0.], 10., 15., 1.5);
            pair(
                doc,
                "beltMount",
                [0., 14.350_440_756_036_2],
                if edit == Some(1) { 27. } else { 25. },
                3.5,
                0,
            );
            assert_circle_enclosure(&native);
            let profile = polygon(&native);
            assert_eq!(profile.len(), 8);
            assert!(contains(&profile, [0., 0.]));
            assert!(!contains(&profile, [18., 0.]));
            assert!(contains(&profile, [18., 20.]));
        }
        "voron-panel" => {
            let profile = polygon(&native);
            assert_eq!(profile.len(), 20);
            for p in &profile {
                assert!(
                    profile
                        .iter()
                        .any(|q| (p[0] + q[0]).abs() < 1e-8 && (p[1] - q[1]).abs() < 1e-8),
                    "bilateral outline symmetry"
                );
            }
            near(
                profile.iter().map(|p| p[0].abs()).fold(0., f64::max),
                if edit == Some(0) { 63. } else { 61. },
            );
            near(profile.iter().map(|p| p[1].abs()).fold(0., f64::max), 18.5);
            for p in [[0., 0.], [0., 17.], [-59., 2.], [59., 2.]] {
                assert!(
                    !contains(&profile, p),
                    "opening incorrectly contains material at {p:?}"
                );
            }
            assert!(contains(&profile, [30., 0.]));
            let cross = if edit == Some(1) { 17. } else { 15. };
            assert!(
                profile
                    .iter()
                    .any(|p| (p[0] - cross).abs() < 1e-8 && (p[1] - 5.).abs() < 1e-8)
            );
        }
        "bondtech-indx-link" => {
            let seats = named_circles(doc, "")
                .into_iter()
                .filter(|(_, r)| (*r - if edit == Some(1) { 2.7 } else { 2.5 }).abs() < 1e-8)
                .collect::<Vec<_>>();
            assert_eq!(seats.len(), 3);
            let pitch = if edit == Some(0) { 13. } else { 14. };
            for (p, r) in &seats {
                near(p[0].hypot(p[1]), pitch);
                near(*r, if edit == Some(1) { 2.7 } else { 2.5 });
                assert!(
                    p[0].abs() + r < 37.858 && p[1].abs() + r < 16.773,
                    "demonstrated coupling edits fit the published plan reference"
                );
            }
            for i in 0..3 {
                let p = seats[i].0;
                let q = seats[(i + 1) % 3].0;
                near((p[0] - q[0]).hypot(p[1] - q[1]), 3_f64.sqrt() * pitch);
            }
            let board = named_circles(doc, "boardMountUpper");
            assert_eq!(board.len(), 1);
            near(board[0].0[0], 66.);
            near(board[0].0[1], 14.);
            near(board[0].1, 1.6);
        }
        _ => panic!("unknown audit sample"),
    }
}

fn assert_circle_enclosure(doc: &SketchDocument) {
    let outline = polygon(doc);
    for (point, radius) in circles(doc) {
        for delta in [[radius, 0.], [-radius, 0.], [0., radius], [0., -radius]] {
            let edge = [point[0] + delta[0], point[1] + delta[1]];
            assert!(
                contains(&outline, edge),
                "schematic body excludes interface edge {edge:?}"
            );
        }
    }
}
#[test]
fn micron_counterbores_fit_the_schematic_body() {
    assert_circle_enclosure(&document("micron-carriage"));
}

fn measured_edit(key: &str, index: usize) {
    let sample = bundled_sample(key).unwrap();
    let base = sample.project();
    let input = geosolve_headless::HeadlessInput::BundledSample(key.into());
    let before = geosolve_headless::render(&input).unwrap();
    audit::assert_valid(&before);
    intent(&base, key, None);
    let path = format!(
        "{}/../geosolve-sketch-code/assets/bundled-samples/{key}/audit-edits.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let edits: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let e = &edits["edits"][index];
    let fields = e["path"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect::<Vec<_>>();
    let after = audit::edit_mm(
        &input,
        e["declaration"].as_str().unwrap(),
        &fields,
        e["replacement"]["value"].as_f64().unwrap(),
    );
    audit::assert_valid(&after);
    assert_ne!(after.report().source_digest, before.report().source_digest);
    assert_eq!(after.report().validation.numerical_right_nullity, 0);
    assert_eq!(after.report().validation.equality_degrees_of_freedom, 0);
    assert_eq!(
        after
            .report()
            .validation
            .bidirectional_bounded_degrees_of_freedom,
        0
    );
    intent(after.project(), key, Some(index));
    audit::assert_history(&base, after.project());
    audit::preserve(&before, key, &format!("edit-{index}-before"));
    audit::preserve(&after, key, &format!("edit-{index}-after"));
}
macro_rules! edit_case {
    ($name:ident,$key:literal,$index:literal) => {
        #[test]
        #[ignore = "requires the built pinned TypeScript mutation sidecar"]
        fn $name() {
            measured_edit($key, $index);
        }
    };
}
edit_case!(micron_rail_pitch_edit, "micron-carriage", 0);
edit_case!(micron_belt_pitch_edit, "micron-carriage", 1);
edit_case!(voron_panel_width_edit, "voron-panel", 0);
edit_case!(voron_t_passage_width_edit, "voron-panel", 1);
edit_case!(bondtech_coupling_pitch_edit, "bondtech-indx-link", 0);
edit_case!(bondtech_coupling_seat_edit, "bondtech-indx-link", 1);
