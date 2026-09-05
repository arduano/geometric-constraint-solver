// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use geosolve_constraint_editor::{IntentNativeBinding, Viewport};
use geosolve_headless::{HeadlessInput, inspect};
use geosolve_sketch::{
    CurveDefinition, CurveId, DesignPointId, DocumentId, PersistentId, SketchDocument,
};
use geosolve_sketch_code::{
    CodeProject, ManagedControlEdit, ManagedControlEditBatch, ManagedPathSegment, ManagedValue,
    bundled_sample, materialize_code_project_cold,
};
use geosolve_sketch_intent::IntentSessionId;

#[path = "support/m92_audit.rs"]
mod audit;

const TOLERANCE: f64 = 1e-7;

fn close(actual: f64, expected: f64) {
    assert!(
        actual.is_finite() && (actual - expected).abs() <= TOLERANCE,
        "expected {expected}, got {actual}"
    );
}

fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn same_point(a: [f64; 2], b: [f64; 2]) -> bool {
    distance(a, b) <= TOLERANCE
}

struct Bend {
    center: [f64; 2],
    radius: f64,
    contacts: [(geosolve_sketch::CurveSpan, [f64; 2]); 2],
}

struct Geometry {
    document: SketchDocument,
    owners: BTreeMap<String, Vec<CurveId>>,
    point_ports: BTreeMap<String, BTreeSet<DesignPointId>>,
    reference_values: BTreeMap<String, f64>,
    bends: Vec<Bend>,
}

impl Geometry {
    fn read(project: &CodeProject) -> Self {
        let materialized = materialize_code_project_cold(
            project,
            &audit::generated(project),
            IntentSessionId::from_raw(0x92_a0),
            DocumentId(PersistentId::from_u128(0x92_a0)),
            1.0,
        )
        .unwrap();
        let coordinator = materialized.editor.coordinator();
        let accepted = coordinator.accepted_materialization().unwrap();
        assert!(accepted.validation.hard_residuals_validated);
        assert!(accepted.validation.all_active_features_current);
        assert!(
            accepted
                .validation
                .maximum_normalized_hard_residual
                .is_none_or(|v| v.is_finite() && v <= 1e-9)
        );
        let document = accepted
            .session
            .accepted_state_for_current_input()
            .unwrap()
            .document()
            .clone();
        assert!(
            document
                .points()
                .iter()
                .flat_map(|p| p.position)
                .all(f64::is_finite)
        );
        assert!(document.scalars().iter().all(|s| s.value.is_finite()));
        let mut owners = BTreeMap::<String, Vec<CurveId>>::new();
        for curve in document.curves() {
            let owner = accepted
                .ownership
                .exact_owner(IntentNativeBinding::Curve(curve.id))
                .unwrap();
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
                .unwrap();
            owners.entry(declaration).or_default().push(curve.id);
        }
        let mut point_ports = BTreeMap::<String, BTreeSet<DesignPointId>>::new();
        let mut reference_values = BTreeMap::new();
        let state = accepted.session.accepted_state_for_current_input().unwrap();
        for (alias, declaration) in &materialized.expansion.declaration_provenance {
            let node = coordinator.intent().graph().node_by_symbol(alias).unwrap();
            for port in node.ports.values() {
                match accepted.ownership.port(port.as_ref(node.id)) {
                    Some(IntentNativeBinding::Point(id)) => {
                        point_ports
                            .entry(declaration.0.clone())
                            .or_default()
                            .insert(id);
                    }
                    Some(IntentNativeBinding::Dimension(id)) => {
                        if let Some(value) = state.reference_value(id) {
                            reference_values.insert(declaration.0.clone(), value);
                        }
                    }
                    _ => {}
                }
            }
        }
        let scene = materialized
            .editor
            .scene(
                Viewport::new([1000.0, 700.0], [0.0, 0.0], 1.0).unwrap(),
                0.25,
            )
            .unwrap();
        let bends = scene
            .computed_curves
            .iter()
            .map(|c| Bend {
                center: c.center,
                radius: c.radius,
                contacts: c.contacts.map(|p| (p.source.span, p.position)),
            })
            .collect();
        Self {
            document,
            owners,
            point_ports,
            reference_values,
            bends,
        }
    }

    fn curve(&self, owner: &str) -> CurveId {
        let [curve] = self.owners[owner].as_slice() else {
            panic!("one curve for {owner}")
        };
        *curve
    }

    fn point(&self, owner: &str) -> [f64; 2] {
        let points = &self.point_ports[owner];
        assert_eq!(points.len(), 1, "one point for {owner}");
        self.document
            .point(*points.first().unwrap())
            .unwrap()
            .position
    }

    fn line(&self, curve: CurveId) -> [[f64; 2]; 2] {
        assert!(matches!(
            self.document.curve(curve).unwrap().definition,
            CurveDefinition::Line { .. }
        ));
        [self.endpoint(curve, 0.0), self.endpoint(curve, 1.0)]
    }

    fn assert_on_bounded_line(&self, curve: CurveId, point: [f64; 2]) -> f64 {
        let [a, b] = self.line(curve);
        let length = distance(a, b);
        assert!(length > TOLERANCE);
        let direction = [(b[0] - a[0]) / length, (b[1] - a[1]) / length];
        let delta = [point[0] - a[0], point[1] - a[1]];
        close(delta[0] * direction[1] - delta[1] * direction[0], 0.0);
        let along = delta[0] * direction[0] + delta[1] * direction[1];
        assert!(
            along >= -TOLERANCE && along <= length + TOLERANCE,
            "point lies on its finite parent"
        );
        along
    }

    fn endpoint(&self, curve: CurveId, parameter: f64) -> [f64; 2] {
        let span = self.document.curve_spans(curve).unwrap()[0];
        let point = self
            .document
            .evaluate_curve_jet(span, parameter)
            .unwrap()
            .position;
        [point.x, point.y]
    }

    fn circle(&self, curve: CurveId) -> ([f64; 2], f64) {
        let CurveDefinition::Circle { center, radius } =
            self.document.curve(curve).unwrap().definition
        else {
            panic!("expected circle")
        };
        (
            self.document.point(center).unwrap().position,
            self.document.scalar(radius).unwrap().value,
        )
    }

    fn assert_curves_atlas(&self, roller_radius: f64, trim_end: f64) {
        let (center, radius) = self.circle(self.curve("rollingCircle"));
        let rail = self.curve("tangentRail");
        let [start, end] = [self.endpoint(rail, 0.0), self.endpoint(rail, 1.0)];
        close(radius, roller_radius);
        close(start[1], 3.0);
        close(end[1], 3.0);
        close(end[0] - start[0], 12.0);
        close(center[1] - start[1], radius);
        assert!(
            center[0] > start[0] && center[0] < end[0],
            "interior left-side contact"
        );
        close(distance(self.point("contactWitness"), center), radius);
        let incoming = self.document.curve_spans(self.curve("c2Incoming")).unwrap()[0];
        let outgoing = self.document.curve_spans(self.curve("c2Outgoing")).unwrap()[0];
        let a = self.document.evaluate_curve_jet(incoming, 1.0).unwrap();
        let b = self.document.evaluate_curve_jet(outgoing, 0.0).unwrap();
        close((a.position - b.position).norm(), 0.0);
        close((a.first_derivative - b.first_derivative).norm(), 0.0);
        close((a.second_derivative - b.second_derivative).norm(), 0.0);
        assert!(a.first_derivative.norm() > 10.0, "regular C2 join");
        // Independent focus/directrix parabola formula: p=1, axis +Y, transverse -X.
        let point = self.endpoint(self.curve("parabola"), 1.0);
        close(point[0], 40.0 - 2.0 * trim_end);
        close(point[1], 16.0 + trim_end * trim_end);
        let periodic = self.curve("periodicNurbs");
        let CurveDefinition::Nurbs {
            controls,
            degree,
            form,
            ..
        } = &self.document.curve(periodic).unwrap().definition
        else {
            panic!("periodic rational spline specimen")
        };
        assert_eq!(*degree, 2);
        assert_eq!(controls.len(), 4);
        assert_eq!(*form, geosolve_sketch::DocumentBSplineForm::Periodic);
        let spans = self.document.curve_spans(periodic).unwrap();
        assert_eq!(spans.len(), 4);
        for i in 0..spans.len() {
            let end = self.document.evaluate_curve_jet(spans[i], 1.0).unwrap();
            let start = self
                .document
                .evaluate_curve_jet(spans[(i + 1) % spans.len()], 0.0)
                .unwrap();
            close((end.position - start.position).norm(), 0.0);
            close((end.first_derivative - start.first_derivative).norm(), 0.0);
        }
        // Every advertised specimen must survive into a visible, nondegenerate native interval.
        for owner in [
            "datumLine",
            "referenceCircle",
            "circularArc",
            "referenceEllipse",
            "ellipticalArc",
            "rationalConic",
            "parabola",
            "hyperbola",
            "quadraticWave",
            "cubicWave",
            "openSpline",
            "periodicNurbs",
            "c2Incoming",
            "c2Outgoing",
        ] {
            let curve = self.curve(owner);
            let intervals = self.document.visible_curve_intervals(curve).unwrap();
            assert!(!intervals.is_empty(), "{owner} must have visible geometry");
            for interval in intervals {
                assert!(interval.end > interval.start);
                for t in [
                    interval.start,
                    interval.start.midpoint(interval.end),
                    interval.end,
                ] {
                    let jet = self
                        .document
                        .evaluate_curve_jet(interval.support, t)
                        .unwrap();
                    assert!(jet.position.iter().all(|v| v.is_finite()));
                    assert!(jet.first_derivative.norm() > TOLERANCE);
                }
            }
        }
    }

    fn assert_operations(&self, width: f64, fillet_radius: f64) {
        let rectangle = &self.owners["stockBlank"];
        assert_eq!(rectangle.len(), 4);
        let mut lengths = rectangle
            .iter()
            .map(|id| {
                let [a, b] = [self.endpoint(*id, 0.0), self.endpoint(*id, 1.0)];
                (a[0] - b[0]).hypot(a[1] - b[1])
            })
            .collect::<Vec<_>>();
        lengths.sort_by(f64::total_cmp);
        for (actual, expected) in lengths.into_iter().zip([10.0, 10.0, width, width]) {
            close(actual, expected);
        }
        self.assert_hexagon();
        self.assert_obround();
        self.assert_chamfer_and_offset();
        self.assert_pattern();
        self.assert_metrology();
        let fillet = self.curve("cornerFillet");
        let CurveDefinition::CircularArc {
            center,
            radius,
            sweep,
            ..
        } = self.document.curve(fillet).unwrap().definition
        else {
            panic!("native fillet arc")
        };
        assert_eq!(sweep, geosolve_sketch::DocumentArcSweep::CounterClockwise);
        close(self.document.scalar(radius).unwrap().value, fillet_radius);
        let center = self.document.point(center).unwrap().position;
        for (parent, t) in [("filletHorizontal", 0.0), ("filletVertical", 1.0)] {
            let contact = self.endpoint(fillet, t);
            let line = self.curve(parent);
            let [a, b] = [self.endpoint(line, 0.0), self.endpoint(line, 1.0)];
            let tangent = [b[0] - a[0], b[1] - a[1]];
            let length = tangent[0].hypot(tangent[1]);
            let along = self.assert_on_bounded_line(line, contact);
            assert!(
                along > TOLERANCE && along < length - TOLERANCE,
                "interior fillet contact"
            );
            close(
                (tangent[0] * (center[1] - contact[1]) - tangent[1] * (center[0] - contact[0]))
                    / length,
                fillet_radius,
            );
            close(
                ((contact[0] - a[0]) * tangent[1] - (contact[1] - a[1]) * tangent[0]) / length,
                0.0,
            );
            close(
                ((contact[0] - center[0]) * tangent[0] + (contact[1] - center[1]) * tangent[1])
                    / length,
                0.0,
            );
        }
        for (owner, expected) in [
            ("splitSource", vec![(0.0, 0.4), (0.4, 1.0)]),
            ("breakSource", vec![(0.0, 0.3), (0.7, 1.0)]),
            ("trimSource", vec![(0.65, 1.0)]),
        ] {
            let intervals = self
                .document
                .visible_curve_intervals(self.curve(owner))
                .unwrap();
            assert_eq!(intervals.len(), expected.len());
            for (interval, (start, end)) in intervals.iter().zip(expected) {
                close(interval.start, start);
                close(interval.end, end);
            }
        }
        close(self.endpoint(self.curve("extensionSource"), 1.0)[0], 34.0);
        let seed = self.curve("mirrorSeed");
        let reflection = self.curve("reflectedProfile");
        for t in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let [a, b] = [self.endpoint(seed, t), self.endpoint(reflection, t)];
            close(a[0] + b[0], -56.0);
            close(a[1], b[1]);
        }
    }

    fn assert_hexagon(&self) {
        let sides = &self.owners["hexFlange"];
        assert_eq!(sides.len(), 6);
        let vertices = (0..6)
            .map(|index| {
                let angle = (30.0 + 60.0 * f64::from(index)).to_radians();
                [-13.0 + 6.0 * angle.cos(), 20.0 + 6.0 * angle.sin()]
            })
            .collect::<Vec<_>>();
        let lines = sides.iter().map(|id| self.line(*id)).collect::<Vec<_>>();
        for i in 0..6 {
            assert_eq!(
                lines
                    .iter()
                    .filter(|[a, b]| same_point(*a, vertices[i])
                        && same_point(*b, vertices[(i + 1) % 6]))
                    .count(),
                1,
                "one correctly oriented hexagon edge at each circumcircle vertex"
            );
        }
        for [a, b] in lines {
            close(distance(a, [-13.0, 20.0]), 6.0);
            close(distance(a, b), 6.0);
        }
    }

    fn assert_obround(&self) {
        let curves = &self.owners["obroundPort"];
        assert_eq!(curves.len(), 4);
        let mut lines = Vec::new();
        let mut arcs = Vec::new();
        for id in curves {
            match self.document.curve(*id).unwrap().definition {
                CurveDefinition::Line { .. } => lines.push(*id),
                CurveDefinition::CircularArc {
                    center,
                    radius,
                    sweep,
                    ..
                } => {
                    arcs.push((*id, self.document.point(center).unwrap().position));
                    close(self.document.scalar(radius).unwrap().value, 3.0);
                    assert_eq!(sweep, geosolve_sketch::DocumentArcSweep::Clockwise);
                }
                _ => panic!("obround contains only two lines and two circular arcs"),
            }
        }
        assert_eq!((lines.len(), arcs.len()), (2, 2));
        for center in [[4.0, 20.0], [16.0, 20.0]] {
            assert_eq!(
                arcs.iter().filter(|(_, p)| same_point(*p, center)).count(),
                1
            );
        }
        for (arc, center) in arcs {
            let span = self.document.curve_spans(arc).unwrap()[0];
            let midpoint = self.endpoint(arc, 0.5);
            close(midpoint[0], if center[0] < 10.0 { 1.0 } else { 19.0 });
            close(midpoint[1], 20.0);
            for t in [0.0, 1.0] {
                let contact = self.document.evaluate_curve_jet(span, t).unwrap();
                let p = [contact.position.x, contact.position.y];
                let matches = lines
                    .iter()
                    .filter(|line| {
                        // The slot traverses clockwise: arc end joins line start and vice versa.
                        same_point(self.endpoint(**line, 1.0 - t), p)
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    matches.len(),
                    1,
                    "each arc endpoint closes exactly one line endpoint"
                );
                let [a, b] = self.line(*matches[0]);
                close(distance(a, b), 12.0);
                close(a[1], b[1]);
                close(
                    (p[0] - center[0]) * (b[0] - a[0]) + (p[1] - center[1]) * (b[1] - a[1]),
                    0.0,
                );
                let tangent = contact.first_derivative.normalize();
                close(
                    (tangent.x * (b[0] - a[0]) + tangent.y * (b[1] - a[1])) / distance(a, b),
                    1.0,
                );
            }
        }
    }

    fn assert_chamfer_and_offset(&self) {
        let chamfer = self.line(self.curve("cornerChamfer"));
        let first = self.curve("chamferHorizontal");
        let second = self.curve("chamferVertical");
        let corner = self.endpoint(first, 0.0);
        close(distance(corner, self.endpoint(second, 0.0)), 0.0);
        for (parent, contact, setback) in [(first, chamfer[0], 2.0), (second, chamfer[1], 3.0)] {
            close(self.assert_on_bounded_line(parent, contact), setback);
            close(distance(corner, contact), setback);
        }
        close(distance(chamfer[0], chamfer[1]), 13.0_f64.sqrt());
        let source = self.line(self.curve("offsetSource"));
        let output = self.line(self.curve("machiningAllowance"));
        let length = distance(source[0], source[1]);
        let d = [
            (source[1][0] - source[0][0]) / length,
            (source[1][1] - source[0][1]) / length,
        ];
        close(distance(output[0], output[1]), length);
        for i in 0..2 {
            let translation = [output[i][0] - source[i][0], output[i][1] - source[i][1]];
            close(translation[0] * d[0] + translation[1] * d[1], 0.0);
            close(d[0] * translation[1] - d[1] * translation[0], 2.0);
        }
    }

    fn assert_pattern(&self) {
        let copies = &self.owners["holeStrip"];
        assert_eq!(copies.len(), 46, "23 additional two-span crosses");
        let lines = copies
            .iter()
            .copied()
            .chain([self.curve("pilotHorizontal"), self.curve("pilotVertical")])
            .map(|id| self.line(id))
            .collect::<Vec<_>>();
        assert_eq!(lines.len(), 48);
        for index in 0..24 {
            let x = -38.0 + 3.3 * f64::from(index);
            for expected in [
                [[x - 1.4, -24.0], [x + 1.4, -24.0]],
                [[x, -25.4], [x, -22.6]],
            ] {
                assert_eq!(
                    lines
                        .iter()
                        .filter(|[a, b]| same_point(*a, expected[0]) && same_point(*b, expected[1]))
                        .count(),
                    1,
                    "exactly one correctly placed/oriented marker arm per instance"
                );
            }
        }
    }

    fn assert_metrology(&self) {
        for (baseline, upright) in [
            ("datumBaseline", "datumUpright"),
            ("followerBaseline", "followerUpright"),
        ] {
            let [a, b] = self.line(self.curve(baseline));
            let [c, d] = self.line(self.curve(upright));
            close(distance(a, c), 0.0);
            close(b[0] - a[0], 14.0);
            close(b[1] - a[1], 0.0);
            close(d[0] - c[0], 0.0);
            close(d[1] - c[1], 10.0);
        }
        let [a, b] = self.line(self.curve("datumBaseline"));
        close(distance(a, [-38.0, -39.0]), 0.0);
        close(
            distance(
                self.point("datumMidpoint"),
                [a[0].midpoint(b[0]), a[1].midpoint(b[1])],
            ),
            0.0,
        );
        let (first, r1) = self.circle(self.curve("inspectionBore"));
        let (second, r2) = self.circle(self.curve("comparisonBore"));
        close(first[1], second[1]);
        close(r1, 3.0);
        close(r2, r1);
        for (declaration, expected) in [
            ("datumAngle", std::f64::consts::FRAC_PI_2),
            ("comparisonRadius", r2),
            (
                "seedChord",
                distance(
                    self.endpoint(self.curve("splitSource"), 0.0),
                    self.endpoint(self.curve("splitSource"), 1.0),
                ),
            ),
            ("markerEdge", 2.8),
        ] {
            close(self.reference_values[declaration], expected);
        }
    }

    fn assert_field(&self, north_pilot: f64, north_counterbore: f64) {
        let mut all_centers = Vec::new();
        for (owner, count, pilot, counterbore) in [
            ("northernCells", 48, north_pilot, north_counterbore),
            ("centralCells", 96, 2.5, 4.5),
            ("southernCells", 48, 2.5, 4.5),
        ] {
            let mut cells = BTreeMap::<_, Vec<f64>>::new();
            for id in &self.owners[owner] {
                let CurveDefinition::Circle { center, radius } =
                    self.document.curve(*id).unwrap().definition
                else {
                    panic!("cell circle")
                };
                cells
                    .entry(center)
                    .or_default()
                    .push(self.document.scalar(radius).unwrap().value);
            }
            assert_eq!(cells.len(), count);
            for (center, mut radii) in cells {
                radii.sort_by(f64::total_cmp);
                assert_eq!(
                    radii.len(),
                    2,
                    "shared native centre, pilot and counterbore"
                );
                close(radii[0], pilot);
                close(radii[1], counterbore);
                all_centers.push(self.document.point(center).unwrap().position);
            }
        }
        assert_eq!(all_centers.len(), 192);
        for row in 0..8 {
            for column in 0..24 {
                let expected = [
                    -172.5 + 15.0 * f64::from(column),
                    52.5 - 15.0 * f64::from(row),
                ];
                assert_eq!(
                    all_centers
                        .iter()
                        .filter(|p| (p[0] - expected[0]).abs() < TOLERANCE
                            && (p[1] - expected[1]).abs() < TOLERANCE)
                        .count(),
                    1,
                    "complete 24 × 8 lattice"
                );
            }
        }
    }

    fn assert_harness(&self, clip_radius: f64, bend_radius: f64) {
        let routes = self
            .document
            .curves()
            .iter()
            .filter_map(|curve| match &curve.definition {
                CurveDefinition::Polyline {
                    points,
                    closed: false,
                    ..
                } => Some(points),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(routes.len(), 8);
        assert!(routes.iter().all(|route| route.len() == 10));
        let vertices = routes
            .iter()
            .flat_map(|route| route.iter())
            .copied()
            .collect::<Vec<_>>();
        let mut clips = 0;
        let mut mounts = 0;
        for curve in self.document.curves() {
            if let CurveDefinition::Circle { center, radius } = curve.definition {
                let expected = if vertices.contains(&center) {
                    clips += 1;
                    clip_radius
                } else {
                    mounts += 1;
                    4.0
                };
                close(self.document.scalar(radius).unwrap().value, expected);
            }
        }
        assert_eq!((clips, mounts), (80, 8));
        assert_eq!(self.bends.len(), 64);
        let mut corners = std::collections::BTreeSet::new();
        for bend in &self.bends {
            close(bend.radius, bend_radius);
            let [(first, _), (second, _)] = bend.contacts;
            assert_eq!(first.curve, second.curve, "bend belongs to one route");
            assert_ne!(first, second, "bend parents must be distinct");
            let spans = self.document.curve_spans(first.curve).unwrap();
            let first_index = spans.iter().position(|span| *span == first).unwrap();
            let second_index = spans.iter().position(|span| *span == second).unwrap();
            assert_eq!(
                first_index.abs_diff(second_index),
                1,
                "adjacent route parents"
            );
            assert!(
                corners.insert((first.curve, first_index.min(second_index))),
                "one bend per corner"
            );
            for (span, contact) in bend.contacts {
                close(
                    (contact[0] - bend.center[0]).hypot(contact[1] - bend.center[1]),
                    bend_radius,
                );
                let a = self
                    .document
                    .evaluate_curve_jet(span, 0.0)
                    .unwrap()
                    .position;
                let b = self
                    .document
                    .evaluate_curve_jet(span, 1.0)
                    .unwrap()
                    .position;
                let d = b - a;
                let length = d.norm();
                let along = ((contact[0] - a.x) * d.x + (contact[1] - a.y) * d.y) / length;
                close(
                    ((contact[0] - a.x) * d.y - (contact[1] - a.y) * d.x) / length,
                    0.0,
                );
                close(
                    ((contact[0] - bend.center[0]) * d.x + (contact[1] - bend.center[1]) * d.y)
                        / length,
                    0.0,
                );
                assert!(
                    along >= -TOLERANCE && along <= length + TOLERANCE,
                    "bounded owning parent"
                );
            }
        }
        assert_eq!(corners.len(), 64, "all eight corners of each route");
    }
}

#[test]
fn curves_atlas_geometry_fulfills_declared_intent() {
    Geometry::read(
        &bundled_sample("curves-contact-continuity-atlas")
            .unwrap()
            .project(),
    )
    .assert_curves_atlas(3.0, 2.0);
}

#[test]
fn operations_atlas_geometry_fulfills_declared_intent() {
    Geometry::read(
        &bundled_sample("fabrication-operations-atlas")
            .unwrap()
            .project(),
    )
    .assert_operations(14.0, 2.0);
}

#[test]
fn fixture_field_geometry_fulfills_declared_intent() {
    Geometry::read(
        &bundled_sample("perforated-fixture-field")
            .unwrap()
            .project(),
    )
    .assert_field(2.5, 4.5);
}

#[test]
fn harness_geometry_fulfills_declared_intent() {
    Geometry::read(
        &bundled_sample("robotic-harness-backplane")
            .unwrap()
            .project(),
    )
    .assert_harness(2.4, 5.0);
}

#[test]
fn harness_mounting_holes_are_separate_from_route_clips() {
    let geometry = Geometry::read(
        &bundled_sample("robotic-harness-backplane")
            .unwrap()
            .project(),
    );
    for (owner, ids) in &geometry.owners {
        if !owner.ends_with("Mount") {
            continue;
        }
        for mount in ids {
            let (mount_center, mount_radius) = geometry.circle(*mount);
            for (clip_owner, clip_ids) in &geometry.owners {
                if !clip_owner.ends_with("Harness") {
                    continue;
                }
                for clip in clip_ids {
                    let (clip_center, clip_radius) = geometry.circle(*clip);
                    let separation =
                        (mount_center[0] - clip_center[0]).hypot(mount_center[1] - clip_center[1]);
                    assert!(
                        separation > mount_radius + clip_radius + 1.0,
                        "{owner} overlaps {clip_owner}: centre separation {separation}, radius sum {}",
                        mount_radius + clip_radius
                    );
                }
            }
        }
    }
}

fn measured_edit(
    key: &str,
    declaration: &str,
    path: &[&str],
    value: f64,
    millimeters: bool,
    check: impl FnOnce(&Geometry),
) {
    let input = HeadlessInput::BundledSample(key.into());
    let edited = if millimeters {
        audit::edit_mm(&input, declaration, path, value)
    } else {
        let inspection = inspect(&input).unwrap();
        let path = path
            .iter()
            .map(|s| ManagedPathSegment::Field((*s).into()))
            .collect::<Vec<_>>();
        let control = inspection
            .controls
            .editable()
            .find(|c| c.source.declaration.0 == declaration && c.source.path.0 == path)
            .unwrap();
        audit::edit_with_pinned_deno(
            &input,
            &ManagedControlEditBatch::new([ManagedControlEdit {
                token: control.token().unwrap().clone(),
                value: ManagedValue::Number(value),
            }]),
        )
        .unwrap()
    };
    audit::assert_valid(&edited);
    check(&Geometry::read(edited.project()));
    audit::assert_history(&bundled_sample(key).unwrap().project(), edited.project());
    audit::preserve(&edited, key, &format!("{declaration}-{}", path.join("-")));
}

#[test]
#[ignore = "requires built package and pinned Deno; mandatory release gate"]
fn curves_atlas_two_edits_preserve_contact_continuity_and_history() {
    let key = "curves-contact-continuity-atlas";
    measured_edit(key, "parabola", &["trimEnd"], 2.5, false, |g| {
        g.assert_curves_atlas(3.0, 2.5);
    });
    measured_edit(key, "rollerRadius", &["value"], 3.5, true, |g| {
        g.assert_curves_atlas(3.5, 2.0);
    });
}

#[test]
#[ignore = "requires built package and pinned Deno; mandatory release gate"]
fn operations_atlas_two_edits_change_measured_outputs_and_preserve_history() {
    let key = "fabrication-operations-atlas";
    measured_edit(key, "cornerFillet", &["radius"], 2.25, true, |g| {
        g.assert_operations(14.0, 2.25);
    });
    measured_edit(key, "stockBlank", &["width"], 16.0, true, |g| {
        g.assert_operations(16.0, 2.0);
    });
}

#[test]
#[ignore = "requires built package and pinned Deno; mandatory release gate"]
fn fixture_field_two_edits_preserve_complete_local_fanout_and_history() {
    let key = "perforated-fixture-field";
    measured_edit(key, "northernCells", &["pilotRadius"], 2.7, true, |g| {
        g.assert_field(2.7, 4.5);
    });
    measured_edit(
        key,
        "northernCells",
        &["counterboreRadius"],
        5.0,
        true,
        |g| g.assert_field(2.5, 5.0),
    );
}

#[test]
#[ignore = "requires built package and pinned Deno; mandatory release gate"]
fn harness_two_edits_preserve_complete_shared_fanout_and_history() {
    let key = "robotic-harness-backplane";
    measured_edit(key, "bendRadius", &[], 4.5, true, |g| {
        g.assert_harness(2.4, 4.5);
    });
    measured_edit(key, "clipRadius", &[], 2.8, true, |g| {
        g.assert_harness(2.8, 5.0);
    });
}
