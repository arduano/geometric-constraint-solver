// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::IntentNativeBinding;
use geosolve_headless::{
    HeadlessInput, HeadlessPreparedEdit, HeadlessRender, inspect, prepare_edit, publish_render,
    render, resolve_edit,
};
use geosolve_sketch::{CurveDefinition, DocumentId, PersistentId};
use geosolve_sketch_code::{
    CodeProject, KeyedReconcileState, ManagedControlEdit, ManagedControlEditBatch,
    ManagedPathSegment, ManagedValue, PreparedManagedMutationReceipt, SketchCodeSession,
    UnitLiteral, bundled_sample, materialize_code_project_cold, required_generated_members,
};
use geosolve_sketch_intent::IntentSessionId;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write as _,
    path::PathBuf,
    process::{Command, Stdio},
};

#[derive(Debug)]
struct Geometry {
    points: Vec<[f64; 2]>,
    radius: Option<f64>,
}
fn geometry(project: &CodeProject) -> BTreeMap<String, Geometry> {
    let generated = KeyedReconcileState::empty()
        .plan(
            required_generated_members(project).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap()
        .into_staged();
    let materialized = materialize_code_project_cold(
        project,
        &generated,
        IntentSessionId::from_raw(0x9260),
        DocumentId(PersistentId::from_u128(0x9260)),
        1.0,
    )
    .unwrap();
    let coordinator = materialized.editor.coordinator();
    let accepted = coordinator.accepted_materialization().unwrap();
    let state = accepted.session.accepted_state_for_current_input().unwrap();
    let document = state.document();
    assert!(
        accepted.validation.hard_residuals_validated
            && accepted.validation.all_active_features_current
    );
    assert!(
        state
            .solve_result()
            .unstable_core_report()
            .audit
            .sources
            .iter()
            .flat_map(|state| &state.rows)
            .all(|r| r.normalized_residual.is_finite() && r.normalized_residual.abs() <= 1e-9)
    );
    assert!(
        document
            .points()
            .iter()
            .flat_map(|p| p.position)
            .all(f64::is_finite)
    );
    assert!(
        document
            .scalars()
            .iter()
            .all(|state| state.value.is_finite())
    );
    assert_eq!(
        state.diagnostics().rank.unwrap().numerical_right_nullity,
        Some(0)
    );
    let mut result = BTreeMap::<String, Geometry>::new();
    for (alias, decl) in &materialized.expansion.declaration_provenance {
        let node = coordinator.intent().graph().node_by_symbol(alias).unwrap();
        for port in node.ports.values() {
            let Some(IntentNativeBinding::Curve(id)) =
                accepted.ownership.port(port.as_ref(node.id))
            else {
                continue;
            };
            let curve = document.curve(id).unwrap();
            let (ids, radius) = match &curve.definition {
                CurveDefinition::Line { start, end, .. } => (vec![*start, *end], None),
                CurveDefinition::Polyline { points, .. } => (points.clone(), None),
                CurveDefinition::Circle { center, radius } => {
                    (vec![*center], Some(document.scalar(*radius).unwrap().value))
                }
                _ => continue,
            };
            let points = ids
                .into_iter()
                .map(|id| document.point(id).unwrap().position)
                .collect::<Vec<_>>();
            if let Some(existing) = result.get_mut(&decl.0) {
                for point in points {
                    if !existing.points.contains(&point) {
                        existing.points.push(point);
                    }
                }
            } else {
                result.insert(decl.0.clone(), Geometry { points, radius });
            }
        }
    }
    for value in result.values_mut() {
        canonical_rectangle_order(value);
    }
    result
}
fn canonical_rectangle_order(value: &mut Geometry) {
    if value.points.len() == 4 {
        let min = [
            value
                .points
                .iter()
                .map(|p| p[0])
                .fold(f64::INFINITY, f64::min),
            value
                .points
                .iter()
                .map(|p| p[1])
                .fold(f64::INFINITY, f64::min),
        ];
        let max = [
            value
                .points
                .iter()
                .map(|p| p[0])
                .fold(f64::NEG_INFINITY, f64::max),
            value
                .points
                .iter()
                .map(|p| p[1])
                .fold(f64::NEG_INFINITY, f64::max),
        ];
        let corners = vec![min, [max[0], min[1]], max, [min[0], max[1]]];
        if corners.iter().all(|p| value.points.contains(p)) {
            value.points = corners;
        }
    }
}
fn near(actual: f64, expected: f64, context: &str) {
    assert!(
        (actual - expected).abs() < 1e-7,
        "{context}: actual={actual}, expected={expected}"
    );
}
fn mutation(prepared: &HeadlessPreparedEdit) -> PreparedManagedMutationReceipt {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/geosolve-sketch-code");
    let mut child = Command::new("deno")
        .args([
            "run",
            "--no-config",
            "--no-lock",
            "--no-prompt",
            "--cached-only",
            "--no-remote",
            "--node-modules-dir=manual",
            "--ignore-env",
            "scripts/mutate-managed-deno.mjs",
        ])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(prepared).unwrap())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}
fn edit(key: &str, decl: &str, field: &str, value: f64) -> HeadlessRender {
    let input = HeadlessInput::BundledSample(key.to_owned());
    let before = inspect(&input).unwrap();
    let control = before
        .controls
        .editable()
        .find(|c| {
            c.source.declaration.0 == decl
                && c.source.path.0 == vec![ManagedPathSegment::Field(field.into())]
        })
        .unwrap();
    let batch = ManagedControlEditBatch::new([ManagedControlEdit {
        token: control.token().unwrap().clone(),
        value: ManagedValue::Unit(UnitLiteral {
            unit: "mm".into(),
            value,
        }),
    }]);
    let p = prepare_edit(&input, &batch).unwrap();
    let edited = resolve_edit(&input, &p, mutation(&p)).unwrap();
    let reloaded = render(&HeadlessInput::CodeProjectJson(
        edited.project().to_canonical_json().unwrap(),
    ))
    .unwrap();
    assert_eq!(reloaded.project(), edited.project());
    assert_eq!(
        reloaded.report().scene_markup_sha256,
        edited.report().scene_markup_sha256
    );
    assert_history(&bundled_sample(key).unwrap().project(), edited.project());
    if let Ok(root) = std::env::var("M92_PRODUCT_EVIDENCE") {
        std::fs::create_dir_all(&root).unwrap();
        let path = PathBuf::from(root).join(format!(
            "{key}-{decl}-{}",
            &edited.report().source_digest[..12]
        ));
        if !path.exists() {
            publish_render(&edited, &path).unwrap();
        }
    }
    edited
}
#[test]
// M92-F007: independent accepted-geometry regression.
fn dust_shoe_split_crosses_the_screw_lug_and_spindle_bore() {
    let g = geometry(&bundled_sample("dust-shoe-clamp").unwrap().project());
    let split = &g["splitRelief"].points;
    let lug = &g["clampLug"].points;
    assert!(
        split[0][0] < lug[0][0] && split[1][0] > -g["spindleBore"].radius.unwrap(),
        "split must open through the screw lug to the bore; split={split:?}, lug={lug:?}"
    );
}
#[test]
#[ignore = "requires the pinned TypeScript sidecar"]
// M92-F006: independent accepted-geometry regression.
fn gridfinity_height_edit_preserves_floor_and_increases_cavity() {
    let before = geometry(&bundled_sample("gridfinity-bin-section").unwrap().project());
    let edited = edit("gridfinity-bin-section", "bodyRiseLength", "value", 21.0);
    let after = geometry(edited.project());
    near(
        after["section"].points[13][1],
        before["section"].points[13][1],
        "fixed cavity floor",
    );
    near(
        after["section"].points[7][1] - before["section"].points[7][1],
        7.0,
        "lip rises by one unit",
    );
}
#[test]
#[ignore = "requires the pinned TypeScript sidecar"]
// M92-F005: independent accepted-geometry regression.
fn declared_radius_edits_change_accepted_manufacturing_dimensions() {
    for (key, curve, expected) in [
        ("dust-shoe-clamp", "spindleBore", 33.0),
        ("nema-17-motor-interface", "pilot", 11.2),
    ] {
        let witness: serde_json::Value =
            serde_json::from_str(bundled_sample(key).unwrap().witnesses_json()).unwrap();
        let w = &witness["representative_edit"];
        let edited = edit(
            key,
            w["declaration"].as_str().unwrap(),
            w["path"][0].as_str().unwrap(),
            expected,
        );
        let g = geometry(edited.project());
        near(g[curve].radius.unwrap(), expected, key);
    }
}
#[test]
// M92-F008: independent accepted-geometry regression.
fn manifold_seal_encloses_the_shared_reservoir_and_routes() {
    let g = geometry(&bundled_sample("pc-water-manifold").unwrap().project());
    let wet = g
        .iter()
        .filter(|(k, _)| k.as_str() == "reservoir" || k.ends_with("Centerline"))
        .flat_map(|(_, v)| &v.points)
        .collect::<Vec<_>>();
    assert!(
        g.iter().filter(|(k, _)| k.ends_with("Seal")).any(|(_, v)| {
            let min = [
                v.points.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min),
                v.points.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min),
            ];
            let max = [
                v.points
                    .iter()
                    .map(|p| p[0])
                    .fold(f64::NEG_INFINITY, f64::max),
                v.points
                    .iter()
                    .map(|p| p[1])
                    .fold(f64::NEG_INFINITY, f64::max),
            ];
            wet.iter()
                .all(|p| p[0] > min[0] && p[0] < max[0] && p[1] > min[1] && p[1] < max[1])
        }),
        "a closed seal must enclose the complete connected wet circuit instead of traversing the reservoir"
    );
}
#[test]
// M92-F009: independent accepted-geometry regression.
fn vacuum_workholding_holes_remain_outside_the_gasket() {
    let g = geometry(&bundled_sample("vacuum-fixture-plate").unwrap().project());
    let gasket = &g["gasket"].points;
    for key in ["holeNe", "holeNw", "holeSe", "holeSw"] {
        let h = &g[key];
        let p = h.points[0];
        let r = h.radius.unwrap();
        assert!(
            p[0] + r < gasket[0][0]
                || p[0] - r > gasket[1][0]
                || p[1] + r < gasket[0][1]
                || p[1] - r > gasket[2][1],
            "{key} is open inside the gasket or intersects it"
        );
    }
}

fn assert_history(base: &CodeProject, edited: &CodeProject) {
    let g = KeyedReconcileState::empty()
        .plan(required_generated_members(base).unwrap(), &BTreeSet::new())
        .unwrap()
        .into_staged();
    let m = materialize_code_project_cold(
        base,
        &g,
        IntentSessionId::from_raw(0x9261),
        DocumentId(PersistentId::from_u128(0x9261)),
        1.0,
    )
    .unwrap();
    let mut session = SketchCodeSession::new_project(
        base.clone(),
        g,
        m.expansion,
        serde_json::json!({"stage":"base"}),
    )
    .unwrap();
    let before = session.snapshot().clone();
    let plan = session
        .plan_structural_reconciliation(
            session.identity(),
            required_generated_members(edited).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap();
    let m = materialize_code_project_cold(
        edited,
        plan.staged(),
        IntentSessionId::from_raw(0x9262),
        DocumentId(PersistentId::from_u128(0x9262)),
        1.0,
    )
    .unwrap();
    let p = session
        .prepare_project_edit_from_plan(
            session.identity(),
            edited.clone(),
            plan,
            m.expansion,
            serde_json::json!({"stage":"edited"}),
            "Audit meaningful design edit",
        )
        .unwrap();
    session.apply_prepared(p).unwrap();
    let after = session.snapshot().clone();
    session.undo().unwrap().unwrap();
    assert_eq!(session.snapshot(), &before);
    session.redo().unwrap().unwrap();
    assert_eq!(session.snapshot(), &after);
    let wire = session.to_canonical_json().unwrap();
    assert_eq!(
        SketchCodeSession::from_json(&wire)
            .unwrap()
            .to_canonical_json()
            .unwrap(),
        wire
    );
}
#[test]
fn dogbone_overcuts_cover_every_mortise_corner_with_one_cutter() {
    let g = geometry(&bundled_sample("cnc-dogbone-coupon").unwrap().project());
    for prefix in ["press", "nominal", "loose"] {
        let mortise = &g[&format!("{prefix}Mortise")];
        for (i, suffix) in ["Ll", "Lr", "Ur", "Ul"].iter().enumerate() {
            let relief = &g[&format!("{prefix}Relief{suffix}")];
            near(relief.points[0][0], mortise.points[i][0], "relief corner X");
            near(relief.points[0][1], mortise.points[i][1], "relief corner Y");
            near(relief.radius.unwrap(), 3.175, "shared cutter radius");
        }
    }
    for (prefix, clearance) in [("press", -0.4), ("nominal", 0.0), ("loose", 0.4)] {
        let mortise = &g[&format!("{prefix}Mortise")].points;
        let tab = &g[&format!("{prefix}Tab")].points;
        near(
            (mortise[2][1] - mortise[0][1]) - (tab[2][1] - tab[0][1]),
            clearance,
            "fit clearance",
        );
    }
}
#[test]
#[ignore = "requires the pinned TypeScript sidecar"]
fn dogbone_fit_and_tool_edits_preserve_the_other_design_relation() {
    let fit = edit("cnc-dogbone-coupon", "looseMortiseHeight", "value", 18.5);
    let g = geometry(fit.project());
    near(
        g["looseMortise"].points[2][1] - g["looseMortise"].points[0][1],
        18.5,
        "loose fit",
    );
    near(
        g["looseTab"].points[2][1] - g["looseTab"].points[0][1],
        18.0,
        "tab thickness retained",
    );
    let tool = edit("cnc-dogbone-coupon", "cutterRadius", "value", 4.0);
    let g = geometry(tool.project());
    for (name, relief) in g.iter().filter(|(n, _)| n.contains("Relief")) {
        near(relief.radius.unwrap(), 4.0, name);
    }
}
#[test]
#[ignore = "requires the pinned TypeScript sidecar"]
fn gridfinity_width_edit_keeps_plan_section_and_symmetry_consistent() {
    let edited = edit("gridfinity-bin-section", "baseBottomWidth", "value", 37.6);
    let g = geometry(edited.project());
    let section = &g["section"].points;
    let plan = &g["plan"].points;
    near(section[5][0] - section[22][0], 43.5, "outer section width");
    near(plan[1][0] - plan[0][0], 43.5, "plan follows section");
    near(plan[0][0] + plan[1][0], 0.0, "plan symmetry");
    let cavity = &g["planCavity"].points;
    near(
        cavity[1][0] - cavity[0][0],
        41.6,
        "cavity follows section wall",
    );
    near(cavity[0][1] - plan[0][1], 0.95, "plan wall remains fixed");
    near(section[13][1], 7.0, "floor stays at base top");
    near(section[7][1], 25.4, "lip height unchanged");
}
#[test]
#[ignore = "requires the pinned TypeScript sidecar"]
fn second_radius_and_screw_edits_preserve_interface_datums() {
    let before = geometry(&bundled_sample("nema-17-motor-interface").unwrap().project());
    let e = edit("nema-17-motor-interface", "shaftRadius", "value", 3.0);
    let g = geometry(e.project());
    near(g["shaft"].radius.unwrap(), 3.0, "shaft radius");
    for name in ["holeNe", "holeNw", "holeSe", "holeSw", "pilot"] {
        for (a, b) in g[name].points.iter().zip(&before[name].points) {
            near(a[0], b[0], name);
            near(a[1], b[1], name);
        }
        near(g[name].radius.unwrap(), before[name].radius.unwrap(), name);
    }
    let e = edit("dust-shoe-clamp", "screwAxisLength", "value", 22.0);
    let g = geometry(e.project());
    near(g["upperClampScrew"].points[0][1], 11.0, "upper screw");
    near(g["lowerClampScrew"].points[0][1], -11.0, "lower screw");
    near(g["spindleBore"].radius.unwrap(), 32.5, "spindle unchanged");
}
#[test]
#[ignore = "requires the pinned TypeScript sidecar"]
fn manifold_reservoir_and_outlet_edits_retain_the_shared_circuit() {
    let e = edit("pc-water-manifold", "reservoirWidth", "value", 62.0);
    let g = geometry(e.project());
    near(
        g["reservoir"].points[1][0] - g["reservoir"].points[0][0],
        62.0,
        "reservoir width",
    );
    for prefix in ["upper", "middle", "lower"] {
        near(
            g[&format!("{prefix}Centerline")].points[0][0],
            g["reservoir"].points[1][0],
            "connected outlet",
        );
        near(
            g[&format!("{prefix}Outlet")].points[0][0],
            102.0,
            "outlet follows reservoir",
        );
    }
    near(g["commonSeal"].points[1][0], 108.0, "seal tracks extent");
    let e = edit("pc-water-manifold", "upperOutletRadius", "value", 3.5);
    let g = geometry(e.project());
    near(g["upperOutlet"].radius.unwrap(), 3.5, "outlet size");
    near(
        g["middleOutlet"].radius.unwrap(),
        3.0,
        "other outlet stays fixed",
    );
}
#[test]
#[ignore = "requires the pinned TypeScript sidecar"]
fn vacuum_gasket_and_distribution_edits_preserve_centered_clearances() {
    let e = edit("vacuum-fixture-plate", "gasketWidth", "value", 176.0);
    let g = geometry(e.project());
    let p = &g["gasket"].points;
    near(p[1][0] - p[0][0], 176.0, "gasket width");
    near(p[1][0] + p[0][0], 0.0, "gasket remains centered");
    let e = edit("vacuum-fixture-plate", "upperRightX", "target", 45.0);
    let g = geometry(e.project());
    near(g["portUpperRight"].points[0][0], 45.0, "right port pitch");
    near(g["portUpperLeft"].points[0][0], -45.0, "left port pitch");
    near(
        g["centerPort"].points[0][0],
        0.0,
        "central port remains centered",
    );
}
