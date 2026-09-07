// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{IntentNativeBinding, SceneComputedCurve, Viewport};
use geosolve_headless::{
    HeadlessInput, HeadlessPreparedEdit, HeadlessRender, inspect, prepare_edit, publish_render,
    render, resolve_edit,
};
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DocumentArcSweep, DocumentId, GeometryRole, PersistentId,
    SketchDocument,
};
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
    edges: Vec<BoundaryEdge>,
}

#[derive(Clone, Debug)]
struct BoundaryEdge {
    points: Vec<[f64; 2]>,
    signed_area: f64,
    radius: Option<f64>,
}
#[allow(
    clippy::too_many_lines,
    reason = "one accepted-geometry reader keeps residual, mobility and ownership authentication together"
)]
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
    let scene = materialized
        .editor
        .scene(
            Viewport::new([1000.0, 700.0], [0.0, 0.0], 1.0).unwrap(),
            0.25,
        )
        .unwrap();
    let mut result = BTreeMap::<String, Geometry>::new();
    for curve in document.curves() {
        let owner = accepted
            .ownership
            .exact_owner(IntentNativeBinding::Curve(curve.id))
            .expect("native curve owner");
        let declaration = materialized
            .expansion
            .declaration_provenance
            .iter()
            .find_map(|(alias, declaration)| {
                coordinator
                    .intent()
                    .graph()
                    .node_by_symbol(alias)
                    .filter(|node| node.id == owner)
                    .map(|_| declaration.0.clone())
            })
            .expect("managed curve declaration");
        let (ids, radius) = match &curve.definition {
            CurveDefinition::Line { start, end, .. } => (vec![*start, *end], None),
            CurveDefinition::Polyline { points, .. } => (points.clone(), None),
            CurveDefinition::Circle { center, radius } => {
                (vec![*center], Some(document.scalar(*radius).unwrap().value))
            }
            CurveDefinition::CircularArc { .. } => (Vec::new(), None),
            _ => continue,
        };
        let points = ids
            .into_iter()
            .map(|id| document.point(id).unwrap().position)
            .collect::<Vec<_>>();
        let edges = if matches!(curve.definition, CurveDefinition::Circle { .. }) {
            Vec::new()
        } else {
            scene
                .curves
                .iter()
                .filter(|visible| {
                    visible.span.curve == curve.id
                        && visible.role == GeometryRole::Profile
                        && !visible.origin.is_implicit_construction()
                })
                .map(|visible| {
                    boundary_edge(
                        document,
                        visible.span,
                        *visible.screen_parameters.first().unwrap(),
                        *visible.screen_parameters.last().unwrap(),
                    )
                })
                .collect()
        };
        let existing = result.entry(declaration).or_insert_with(|| Geometry {
            points: Vec::new(),
            radius,
            edges: Vec::new(),
        });
        for point in points {
            if !existing.points.contains(&point) {
                existing.points.push(point);
            }
        }
        existing.edges.extend(edges);
    }
    for curve in &scene.computed_curves {
        let invocation = materialized
            .host_outputs
            .iter()
            .find_map(|(address, outputs)| {
                outputs
                    .iter()
                    .any(|output| output.owner == curve.owner)
                    .then_some(&address.invocation)
            })
            .expect("generated Fillet retains its patch invocation owner");
        result
            .get_mut(invocation)
            .expect("Fillet native parent geometry")
            .edges
            .push(computed_boundary_edge(curve));
    }
    for value in result.values_mut() {
        canonical_rectangle_order(value);
    }
    result
}
// Read the accepted native curve parametrization. The area integral is exact for
// the line and circular-arc walls; samples are used only for containment/clearance.
fn boundary_edge(
    document: &SketchDocument,
    span: CurveSpan,
    lower: f64,
    upper: f64,
) -> BoundaryEdge {
    let curve = document.curve(span.curve).unwrap();
    let radius = match curve.definition {
        CurveDefinition::CircularArc { radius, .. } => Some(document.scalar(radius).unwrap().value),
        _ => None,
    };
    let count = if radius.is_some() { 64 } else { 1 };
    let points = (0..=count)
        .map(|i| {
            let point = document
                .evaluate_curve_jet(
                    span,
                    lower + (upper - lower) * f64::from(i) / f64::from(count),
                )
                .unwrap()
                .position;
            [point.x, point.y]
        })
        .collect::<Vec<_>>();
    let start = points[0];
    let end = *points.last().unwrap();
    let signed_area = if let CurveDefinition::CircularArc { center, .. } = curve.definition {
        let center = document.point(center).unwrap().position;
        let tangent = document
            .evaluate_curve_jet(span, lower)
            .unwrap()
            .first_derivative;
        let radius = radius.unwrap();
        let direction =
            ((start[0] - center[0]) * tangent.y - (start[1] - center[1]) * tangent.x).signum();
        let sweep = direction * tangent.norm() * (upper - lower) / radius;
        0.5 * (center[0] * (end[1] - start[1]) - center[1] * (end[0] - start[0])
            + radius * radius * sweep)
    } else {
        0.5 * (start[0] * end[1] - start[1] * end[0])
    };
    BoundaryEdge {
        points,
        signed_area,
        radius,
    }
}

fn computed_boundary_edge(curve: &SceneComputedCurve) -> BoundaryEdge {
    let sweep = match curve.sweep {
        DocumentArcSweep::CounterClockwise => {
            (curve.end_angle - curve.start_angle).rem_euclid(std::f64::consts::TAU)
        }
        DocumentArcSweep::Clockwise => {
            -(curve.start_angle - curve.end_angle).rem_euclid(std::f64::consts::TAU)
        }
    };
    let points = (0..=64)
        .map(|i| {
            let angle = curve.start_angle + sweep * f64::from(i) / 64.0;
            [
                curve.center[0] + curve.radius * angle.cos(),
                curve.center[1] + curve.radius * angle.sin(),
            ]
        })
        .collect::<Vec<_>>();
    let start = points[0];
    let end = *points.last().unwrap();
    let signed_area = 0.5
        * (curve.center[0] * (end[1] - start[1]) - curve.center[1] * (end[0] - start[0])
            + curve.radius * curve.radius * sweep);
    BoundaryEdge {
        points,
        signed_area,
        radius: Some(curve.radius),
    }
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
fn declared_radius_edit_changes_the_accepted_spindle_bore() {
    let key = "dust-shoe-clamp";
    let witness: serde_json::Value =
        serde_json::from_str(bundled_sample(key).unwrap().witnesses_json()).unwrap();
    let w = &witness["representative_edit"];
    let edited = edit(
        key,
        w["declaration"].as_str().unwrap(),
        w["path"][0].as_str().unwrap(),
        33.0,
    );
    let g = geometry(edited.project());
    near(g["spindleBore"].radius.unwrap(), 33.0, key);
}
#[derive(Debug)]
struct BoundaryLoop {
    points: Vec<[f64; 2]>,
    area: f64,
}

fn same_point(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-7
}

fn boundary_loops(mut edges: Vec<BoundaryEdge>) -> Vec<BoundaryLoop> {
    // Every endpoint must have exactly one mate. This rejects open mouths,
    // disconnected wall ends and branching/overlapping reservoir partitions.
    for edge in &edges {
        for point in [edge.points[0], *edge.points.last().unwrap()] {
            let incidence = edges
                .iter()
                .flat_map(|e| [e.points[0], *e.points.last().unwrap()])
                .filter(|candidate| same_point(point, *candidate))
                .count();
            assert_eq!(incidence, 2, "closed two-valent boundary at {point:?}");
        }
    }
    let mut loops = Vec::new();
    while let Some(first) = edges.pop() {
        let start = first.points[0];
        let mut end = *first.points.last().unwrap();
        let mut points = first.points;
        let mut area = first.signed_area;
        while !same_point(start, end) {
            let index = edges
                .iter()
                .position(|edge| {
                    same_point(edge.points[0], end) || same_point(*edge.points.last().unwrap(), end)
                })
                .expect("one continued boundary edge");
            let mut next = edges.remove(index);
            if !same_point(next.points[0], end) {
                next.points.reverse();
                next.signed_area = -next.signed_area;
            }
            end = *next.points.last().unwrap();
            points.extend(next.points.into_iter().skip(1));
            area += next.signed_area;
        }
        loops.push(BoundaryLoop {
            points,
            area: area.abs(),
        });
    }
    loops.sort_by(|a, b| a.area.total_cmp(&b.area));
    loops
}

fn inside(point: [f64; 2], boundary: &BoundaryLoop) -> bool {
    boundary
        .points
        .windows(2)
        .filter(|edge| {
            let [a, b] = [edge[0], edge[1]];
            (a[1] > point[1]) != (b[1] > point[1])
                && point[0] < (b[0] - a[0]) * (point[1] - a[1]) / (b[1] - a[1]) + a[0]
        })
        .count()
        % 2
        == 1
}

fn point_segment_distance(point: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let delta = [b[0] - a[0], b[1] - a[1]];
    let length_squared = delta[0] * delta[0] + delta[1] * delta[1];
    assert!(length_squared > 0.0);
    let parameter = (((point[0] - a[0]) * delta[0] + (point[1] - a[1]) * delta[1])
        / length_squared)
        .clamp(0.0, 1.0);
    (point[0] - a[0] - parameter * delta[0]).hypot(point[1] - a[1] - parameter * delta[1])
}

fn boundary_distance(point: [f64; 2], boundary: &BoundaryLoop) -> f64 {
    boundary
        .points
        .windows(2)
        .map(|edge| point_segment_distance(point, edge[0], edge[1]))
        .fold(f64::INFINITY, f64::min)
}

fn segment_pair_distance(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> f64 {
    let cross = |u: [f64; 2], v: [f64; 2]| u[0] * v[1] - u[1] * v[0];
    let first = [b[0] - a[0], b[1] - a[1]];
    let second = [d[0] - c[0], d[1] - c[1]];
    let offset = [c[0] - a[0], c[1] - a[1]];
    let denominator = cross(first, second);
    if denominator != 0.0
        && (0.0..=1.0).contains(&(cross(offset, second) / denominator))
        && (0.0..=1.0).contains(&(cross(offset, first) / denominator))
    {
        return 0.0;
    }
    [
        point_segment_distance(a, c, d),
        point_segment_distance(b, c, d),
        point_segment_distance(c, a, b),
        point_segment_distance(d, a, b),
    ]
    .into_iter()
    .fold(f64::INFINITY, f64::min)
}

fn boundary_clearance(first: &BoundaryLoop, second: &BoundaryLoop) -> f64 {
    first
        .points
        .windows(2)
        .flat_map(|a| {
            second
                .points
                .windows(2)
                .map(move |b| segment_pair_distance(a[0], a[1], b[0], b[1]))
        })
        .fold(f64::INFINITY, f64::min)
}

#[allow(
    clippy::too_many_lines,
    reason = "one sample contract keeps connected and separate wet areas, groove area and machining clearances together"
)]
fn assert_manifold_profiles(g: &BTreeMap<String, Geometry>, reservoir_width: f64) {
    let mut wet_edges = Vec::new();
    for prefix in ["upper", "middle", "lower", "stair"] {
        let channel = &g[&format!("{prefix}Channel")];
        let mut radii = channel
            .edges
            .iter()
            .filter_map(|edge| edge.radius)
            .collect::<Vec<_>>();
        radii.sort_by(f64::total_cmp);
        let expected = if prefix == "stair" {
            &[2.0, 2.0, 6.0, 6.0, 14.0, 14.0][..]
        } else {
            &[2.0, 2.0, 6.0, 14.0, 14.0][..]
        };
        assert_eq!(
            radii.len(),
            expected.len(),
            "four wall fillets and the requested end caps for {prefix}"
        );
        for (radius, &expected) in radii.into_iter().zip(expected) {
            near(radius, expected, "channel wall/cap radius");
        }
        let source = &g[&format!("{prefix}Centerline")].points;
        assert_eq!(source.len(), 4, "three spans with two bends for {prefix}");
        let inlet = source[0];
        for y in [inlet[1] - 6.0, inlet[1] + 6.0] {
            assert!(
                channel.edges.iter().any(|edge| {
                    same_point(edge.points[0], [inlet[0], y])
                        || same_point(*edge.points.last().unwrap(), [inlet[0], y])
                }),
                "12 mm inlet wall at {prefix}: {y}"
            );
        }
        for source_span in source.windows(2) {
            let station = [
                f64::midpoint(source_span[0][0], source_span[1][0]),
                f64::midpoint(source_span[0][1], source_span[1][1]),
            ];
            let walls = channel
                .edges
                .iter()
                .filter(|edge| {
                    edge.radius.is_none()
                        && (point_segment_distance(
                            station,
                            edge.points[0],
                            *edge.points.last().unwrap(),
                        ) - 6.0)
                            .abs()
                            < 1e-7
                })
                .count();
            assert_eq!(
                walls, 2,
                "two walls 6 mm from each {prefix} centerline span"
            );
        }
        if prefix != "stair" {
            wet_edges.extend(channel.edges.clone());
        }
    }
    for name in [
        "reservoirBottom",
        "reservoirLowerMouth",
        "reservoirLowerBridge",
        "reservoirUpperBridge",
        "reservoirUpperMouth",
        "reservoirTop",
        "reservoirLeft",
    ] {
        wet_edges.extend(g[name].edges.clone());
    }
    let wet = boundary_loops(wet_edges);
    assert_eq!(
        wet.len(),
        1,
        "one connected reservoir and three open water routes"
    );
    // Each 162 mm route loses 32 mm at its two R8 bends and gains 8π mm.
    // Multiplying by the 12 mm width and adding three R6 half-discs gives
    // an area independent of how the patch assembled its native operations.
    near(
        wet[0].area,
        reservoir_width * 84.0 + 4680.0 + 342.0 * std::f64::consts::PI,
        "connected wet area in square millimetres",
    );
    let stair_source = &g["stairCenterline"].points;
    for (actual, expected) in
        stair_source
            .iter()
            .zip([[0.0, -20.0], [30.0, -20.0], [30.0, -2.0], [82.0, -2.0]])
    {
        near(
            actual[0],
            expected[0] + reservoir_width - 60.0,
            "stair X follows reservoir width",
        );
        near(actual[1], expected[1], "stair rise and end levels");
    }
    let stair = boundary_loops(g["stairChannel"].edges.clone());
    assert_eq!(
        stair.len(),
        1,
        "one separate closed point-to-point water route"
    );
    // The 100 mm source loses 32 mm and gains 8π mm through its R8 bends;
    // its two R6 half-disc caps contribute another 36π square millimetres.
    near(
        stair[0].area,
        816.0 + 132.0 * std::f64::consts::PI,
        "separate stair water area in square millimetres",
    );
    assert!(
        stair[0].points.iter().all(|p| !inside(*p, &wet[0]))
            && wet[0].points.iter().all(|p| !inside(*p, &stair[0])),
        "the stair and shared circuit have separate interiors"
    );
    // Include line-interior crossings, not only sampled boundary vertices.
    // These 64-chord arcs deviate by less than 0.002 mm from their exact circles.
    let water_clearance = boundary_clearance(&stair[0], &wet[0]);
    assert!(
        water_clearance > 7.99,
        "stair/shared water clearance: {water_clearance}"
    );
    let seal = &g["commonSealGroove"];
    let mut radii = seal
        .edges
        .iter()
        .filter_map(|edge| edge.radius)
        .collect::<Vec<_>>();
    radii.sort_by(f64::total_cmp);
    assert_eq!(radii.len(), 8, "four inner and four outer seal fillets");
    for (radius, expected) in radii
        .into_iter()
        .zip([3.8, 3.8, 3.8, 3.8, 6.2, 6.2, 6.2, 6.2])
    {
        near(radius, expected, "seal groove wall radius");
    }
    let seal = boundary_loops(seal.edges.clone());
    assert_eq!(seal.len(), 2, "separate closed inner and outer seal walls");
    near(
        seal[1].area - seal[0].area,
        2.4 * (600.0 + 2.0 * (reservoir_width - 60.0) + 10.0 * std::f64::consts::PI),
        "2.4 mm seal groove area in square millimetres",
    );
    for (name, boundary) in [("shared circuit", &wet[0]), ("stair", &stair[0])] {
        assert!(
            boundary.points.iter().all(|p| inside(*p, &seal[0])),
            "inner groove wall encloses the complete {name} wet boundary"
        );
        let wet_clearance = boundary_clearance(boundary, &seal[0]);
        assert!(
            wet_clearance > 2.79,
            "{name} wet/groove clearance: {wet_clearance}"
        );
    }
    // Bores are through-features and may grow independently of the milled pocket.
    // Include their actual radii in sealing clearance, including the 3.5 mm edit.
    for name in [
        "upperOutlet",
        "middleOutlet",
        "lowerOutlet",
        "stairInlet",
        "stairOutlet",
    ] {
        let bore = &g[name];
        assert!(inside(bore.points[0], &seal[0]));
        let clearance = boundary_distance(bore.points[0], &seal[0]) - bore.radius.unwrap();
        assert!(
            clearance > 1.29,
            "{name} bore/groove clearance: {clearance}"
        );
    }
    for (name, center) in [
        ("stairInlet", stair_source[0]),
        ("stairOutlet", stair_source[3]),
    ] {
        let bore = &g[name];
        near(bore.radius.unwrap(), 3.0, "stair through-bore radius");
        assert!(
            same_point(bore.points[0], center),
            "{name} shares its cap center"
        );
        assert!(
            inside(center, &stair[0]),
            "{name} opens into the stair pocket"
        );
        let clearance = boundary_distance(center, &stair[0]) - bore.radius.unwrap();
        assert!(clearance > 2.99, "{name}/pocket clearance: {clearance}");
    }
    let plate = &g["plate"].points;
    assert!(
        seal[1].points.iter().all(|p| {
            p[0] - plate[0][0] > 4.79
                && plate[2][0] - p[0] > 4.79
                && p[1] - plate[0][1] > 4.79
                && plate[2][1] - p[1] > 4.79
        }),
        "outer groove stays inside the plate edge"
    );
    for (name, screw) in g
        .iter()
        .filter(|(name, shape)| name.starts_with("screw") && shape.radius.is_some())
    {
        assert!(
            !inside(screw.points[0], &seal[1]),
            "{name} stays outside seal"
        );
        let clearance = boundary_distance(screw.points[0], &seal[1]) - screw.radius.unwrap();
        assert!(clearance > 2.29, "{name}/groove clearance: {clearance}");
    }
}

#[test]
// M92-F008, extended for native channel and groove area instead of centreline bounds.
fn manifold_seal_encloses_the_shared_reservoir_and_routes() {
    let g = geometry(&bundled_sample("pc-water-manifold").unwrap().project());
    assert_manifold_profiles(&g, 60.0);
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
fn screw_pitch_edit_preserves_the_spindle_bore() {
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
            98.0,
            "outlet follows reservoir",
        );
    }
    near(g["commonSeal"].points[1][0], 108.0, "seal tracks extent");
    assert_manifold_profiles(&g, 62.0);
    let e = edit("pc-water-manifold", "upperOutletRadius", "value", 3.5);
    let g = geometry(e.project());
    near(g["upperOutlet"].radius.unwrap(), 3.5, "outlet size");
    assert_manifold_profiles(&g, 60.0);
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
