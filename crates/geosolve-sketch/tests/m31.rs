// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(clippy::too_many_lines)]

use geosolve_sketch::{
    AlphaScenarioIds, AlphaScenarioKind, ContactNeighborhood, CurveCurveFilletRequest,
    CurveDefinition, CurveFilletParentRequest, CurveSpan, DocumentArcSweep, DocumentBSplineForm,
    DocumentConstraintDefinition, DocumentCurveNormalSide, DocumentDimensionMode,
    DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint, DocumentHyperbolaBranch,
    DocumentTrimParameter, ScalarDomain, ScalarUnit, SketchDocument, VisualProfileCurveFamily,
    VisualProfileGeometryScope, VisualProfileIssueKind, VisualProfileOptions, VisualProfileStatus,
    alpha_scenario,
};

fn scalar(
    document: &mut SketchDocument,
    label: &str,
    value: f64,
    unit: ScalarUnit,
    domain: ScalarDomain,
) -> geosolve_sketch::DesignScalarId {
    document.add_scalar(label, value, unit, domain).unwrap()
}

fn line(
    document: &mut SketchDocument,
    label: &str,
    start: geosolve_sketch::DesignPointId,
    end: geosolve_sketch::DesignPointId,
) -> geosolve_sketch::CurveId {
    let first = document.point(start).unwrap().position;
    let second = document.point(end).unwrap().position;
    let delta = [second[0] - first[0], second[1] - first[1]];
    let length = delta[0].hypot(delta[1]);
    document
        .add_curve(
            label,
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [delta[0] / length, delta[1] / length],
            },
        )
        .unwrap()
}

fn join_point_to_curve_endpoint(
    document: &mut SketchDocument,
    point: geosolve_sketch::DesignPointId,
    span: CurveSpan,
    parameter: f64,
    neighborhood: ContactNeighborhood,
) {
    let contact = document
        .add_curve_contact("endpoint", span, parameter, 0, neighborhood, None)
        .unwrap();
    document
        .add_constraint(
            "endpoint join",
            DocumentConstraintDefinition::PointOnCurve { point, contact },
        )
        .unwrap();
}

fn close_derived_curve(document: &mut SketchDocument, curve: geosolve_sketch::CurveId) {
    let span = CurveSpan::line(curve);
    let first = document.evaluate_curve_jet(span, 0.0).unwrap().position;
    let second = document.evaluate_curve_jet(span, 1.0).unwrap().position;
    let first_point = document
        .add_point("first endpoint", [first.x, first.y])
        .unwrap();
    let second_point = document
        .add_point("second endpoint", [second.x, second.y])
        .unwrap();
    line(document, "closure", second_point, first_point);
    join_point_to_curve_endpoint(document, first_point, span, 0.0, ContactNeighborhood::Start);
    join_point_to_curve_endpoint(document, second_point, span, 1.0, ContactNeighborhood::End);
}

const ALL_PROFILE_FAMILIES: [VisualProfileCurveFamily; 15] = [
    VisualProfileCurveFamily::Line,
    VisualProfileCurveFamily::Polyline,
    VisualProfileCurveFamily::Circle,
    VisualProfileCurveFamily::CircularArc,
    VisualProfileCurveFamily::Ellipse,
    VisualProfileCurveFamily::EllipticalArc,
    VisualProfileCurveFamily::RationalQuadraticConic,
    VisualProfileCurveFamily::Parabola,
    VisualProfileCurveFamily::Hyperbola,
    VisualProfileCurveFamily::QuadraticBezier,
    VisualProfileCurveFamily::CubicBezier,
    VisualProfileCurveFamily::ClampedBSpline,
    VisualProfileCurveFamily::PeriodicBSpline,
    VisualProfileCurveFamily::ClampedNurbs,
    VisualProfileCurveFamily::PeriodicNurbs,
];

#[derive(Clone, Copy)]
struct PairFrame {
    origin: [f64; 2],
    tangent: [f64; 2],
    normal: [f64; 2],
    size: f64,
}

impl PairFrame {
    fn axes(origin: [f64; 2], tangent: [f64; 2], normal: [f64; 2], size: f64) -> Self {
        Self {
            origin,
            tangent,
            normal,
            size,
        }
    }

    fn map(self, point: [f64; 2]) -> [f64; 2] {
        [
            self.origin[0] + self.size * (self.tangent[0] * point[0] + self.normal[0] * point[1]),
            self.origin[1] + self.size * (self.tangent[1] * point[0] + self.normal[1] * point[1]),
        ]
    }

    fn map_homogeneous(self, weighted_point: [f64; 2], weight: f64) -> [f64; 2] {
        [
            self.origin[0] * weight
                + self.size
                    * (self.tangent[0] * weighted_point[0] + self.normal[0] * weighted_point[1]),
            self.origin[1] * weight
                + self.size
                    * (self.tangent[1] * weighted_point[0] + self.normal[1] * weighted_point[1]),
        ]
    }

    fn circle_target_parameter(self) -> f64 {
        (-self.normal[1])
            .atan2(-self.normal[0])
            .rem_euclid(std::f64::consts::TAU)
    }
}

#[derive(Clone, Copy)]
struct PairCurve {
    family: VisualProfileCurveFamily,
    curve: geosolve_sketch::CurveId,
    target_span: CurveSpan,
    target_parameter: f64,
}

fn pair_point(
    document: &mut SketchDocument,
    label: &str,
    frame: PairFrame,
    point: [f64; 2],
) -> geosolve_sketch::DesignPointId {
    document.add_point(label, frame.map(point)).unwrap()
}

fn pair_ratio(
    document: &mut SketchDocument,
    label: &str,
    value: f64,
) -> geosolve_sketch::DesignScalarId {
    scalar(
        document,
        label,
        value,
        ScalarUnit::Parameter,
        ScalarDomain::Bounded {
            lower: f64::from_bits(1),
            upper: 1.0,
        },
    )
}

fn align_periodic_target(
    document: &mut SketchDocument,
    controls: &[geosolve_sketch::DesignPointId],
    span: CurveSpan,
    frame: PairFrame,
) {
    let jet = document.evaluate_curve_jet(span, 0.5).unwrap();
    let speed = jet.first_derivative.norm();
    let current_tangent = [
        jet.first_derivative.x / speed,
        jet.first_derivative.y / speed,
    ];
    let current_normal = [-current_tangent[1], current_tangent[0]];
    let root = [jet.position.x, jet.position.y];
    let transformed = controls
        .iter()
        .map(|control| {
            let point = document.point(*control).unwrap().position;
            let relative = [point[0] - root[0], point[1] - root[1]];
            let along = relative[0] * current_tangent[0] + relative[1] * current_tangent[1];
            let normal = relative[0] * current_normal[0] + relative[1] * current_normal[1];
            [
                frame.origin[0] + along * frame.tangent[0] + normal * frame.normal[0],
                frame.origin[1] + along * frame.tangent[1] + normal * frame.normal[1],
            ]
        })
        .collect::<Vec<_>>();
    for (control, position) in controls.iter().copied().zip(transformed) {
        document.set_point_position(control, position).unwrap();
    }
}

#[allow(clippy::too_many_lines)]
fn add_pair_curve(
    document: &mut SketchDocument,
    family: VisualProfileCurveFamily,
    frame: PairFrame,
    ordinal: usize,
) -> PairCurve {
    let label = format!("pair {ordinal} {family:?}");
    let (curve, target_span, target_parameter) = match family {
        VisualProfileCurveFamily::Line => {
            let start = pair_point(document, "pair line start", frame, [-4.0, 0.0]);
            let end = pair_point(document, "pair line end", frame, [4.0, 0.0]);
            let curve = line(document, &label, start, end);
            (curve, CurveSpan::line(curve), 0.5)
        }
        VisualProfileCurveFamily::Polyline => {
            let points = [
                pair_point(document, "pair polyline start", frame, [-4.0, 0.0]),
                pair_point(document, "pair polyline end", frame, [4.0, 0.0]),
            ];
            let curve = document
                .add_curve(
                    &label,
                    CurveDefinition::Polyline {
                        points: points.to_vec(),
                        closed: false,
                        branch_directions: vec![frame.tangent],
                    },
                )
                .unwrap();
            (curve, CurveSpan::line(curve), 0.5)
        }
        VisualProfileCurveFamily::Circle => {
            let center = pair_point(document, "pair circle center", frame, [0.0, 1.0]);
            let radius = scalar(
                document,
                "pair circle radius",
                frame.size,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            );
            let curve = document
                .add_curve(&label, CurveDefinition::Circle { center, radius })
                .unwrap();
            (
                curve,
                CurveSpan::line(curve),
                frame.circle_target_parameter(),
            )
        }
        VisualProfileCurveFamily::CircularArc => {
            let center = pair_point(document, "pair arc center", frame, [0.0, 1.0]);
            let radius = scalar(
                document,
                "pair arc radius",
                frame.size,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            );
            let start_angle = scalar(
                document,
                "pair arc start",
                frame.circle_target_parameter() - 0.1 * std::f64::consts::PI,
                ScalarUnit::Angle,
                ScalarDomain::Finite,
            );
            let end_angle = scalar(
                document,
                "pair arc end",
                frame.circle_target_parameter() + 0.1 * std::f64::consts::PI,
                ScalarUnit::Angle,
                ScalarDomain::Finite,
            );
            let curve = document
                .add_curve(
                    &label,
                    CurveDefinition::CircularArc {
                        center,
                        radius,
                        start_angle,
                        end_angle,
                        sweep: DocumentArcSweep::CounterClockwise,
                    },
                )
                .unwrap();
            (curve, CurveSpan::line(curve), 0.5)
        }
        VisualProfileCurveFamily::Ellipse => {
            let center = pair_point(document, "pair ellipse center", frame, [0.0, 1.0]);
            let axis = pair_point(document, "pair ellipse axis", frame, [1.5, 1.0]);
            let ratio = pair_ratio(document, "pair ellipse ratio", 2.0 / 3.0);
            let curve = document
                .add_curve(
                    &label,
                    CurveDefinition::Ellipse {
                        center,
                        major_axis_point: axis,
                        minor_axis_ratio: ratio,
                    },
                )
                .unwrap();
            (curve, CurveSpan::line(curve), 1.5 * std::f64::consts::PI)
        }
        VisualProfileCurveFamily::EllipticalArc => {
            let center = pair_point(document, "pair elliptical center", frame, [0.0, 1.0]);
            let axis = pair_point(document, "pair elliptical axis", frame, [1.5, 1.0]);
            let ratio = pair_ratio(document, "pair elliptical ratio", 2.0 / 3.0);
            let start_angle = scalar(
                document,
                "pair elliptical start",
                -0.75 * std::f64::consts::PI,
                ScalarUnit::Angle,
                ScalarDomain::Finite,
            );
            let end_angle = scalar(
                document,
                "pair elliptical end",
                -0.25 * std::f64::consts::PI,
                ScalarUnit::Angle,
                ScalarDomain::Finite,
            );
            let curve = document
                .add_curve(
                    &label,
                    CurveDefinition::EllipticalArc {
                        center,
                        major_axis_point: axis,
                        minor_axis_ratio: ratio,
                        start_angle,
                        end_angle,
                        sweep: DocumentArcSweep::CounterClockwise,
                    },
                )
                .unwrap();
            (curve, CurveSpan::line(curve), 0.5)
        }
        VisualProfileCurveFamily::RationalQuadraticConic => {
            let start = pair_point(document, "pair rational start", frame, [-2.0, 0.1]);
            let end = pair_point(document, "pair rational end", frame, [2.0, 0.1]);
            let weight = scalar(
                document,
                "pair rational weight",
                0.75,
                ScalarUnit::Parameter,
                ScalarDomain::Bounded {
                    lower: geosolve_sketch::MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
                    upper: f64::MAX,
                },
            );
            let curve = document
                .add_curve(
                    &label,
                    CurveDefinition::RationalQuadraticConic {
                        start,
                        weighted_middle: frame.map_homogeneous([0.0, -0.1], 0.75),
                        middle_weight: weight,
                        end,
                    },
                )
                .unwrap();
            (curve, CurveSpan::line(curve), 0.5)
        }
        VisualProfileCurveFamily::Parabola => {
            let vertex = pair_point(document, "pair parabola vertex", frame, [0.0, 0.0]);
            let focus = pair_point(document, "pair parabola focus", frame, [0.0, 0.5]);
            let trim_start = scalar(
                document,
                "pair parabola start",
                -1.0,
                ScalarUnit::Parameter,
                ScalarDomain::Finite,
            );
            let trim_end = scalar(
                document,
                "pair parabola end",
                1.0,
                ScalarUnit::Parameter,
                ScalarDomain::Finite,
            );
            let curve = document
                .add_curve(
                    &label,
                    CurveDefinition::ParabolaSegment {
                        vertex,
                        focus,
                        trim_start,
                        trim_end,
                    },
                )
                .unwrap();
            (curve, CurveSpan::line(curve), 0.5)
        }
        VisualProfileCurveFamily::Hyperbola => {
            let center = pair_point(document, "pair hyperbola center", frame, [0.0, 1.0]);
            let axis = pair_point(document, "pair hyperbola axis", frame, [0.0, 0.0]);
            let conjugate = scalar(
                document,
                "pair hyperbola conjugate",
                frame.size,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            );
            let trim_start = scalar(
                document,
                "pair hyperbola start",
                -1.0,
                ScalarUnit::Parameter,
                ScalarDomain::Finite,
            );
            let trim_end = scalar(
                document,
                "pair hyperbola end",
                1.0,
                ScalarUnit::Parameter,
                ScalarDomain::Finite,
            );
            let curve = document
                .add_curve(
                    &label,
                    CurveDefinition::HyperbolaSegment {
                        center,
                        transverse_axis_point: axis,
                        semi_conjugate: conjugate,
                        branch: DocumentHyperbolaBranch::Positive,
                        trim_start,
                        trim_end,
                    },
                )
                .unwrap();
            (curve, CurveSpan::line(curve), 0.5)
        }
        VisualProfileCurveFamily::QuadraticBezier => {
            let controls = [[-2.0, 0.1], [0.0, -0.1], [2.0, 0.1]]
                .map(|point| pair_point(document, "pair quadratic control", frame, point));
            let curve = document
                .add_curve(&label, CurveDefinition::QuadraticBezier { controls })
                .unwrap();
            (curve, CurveSpan::line(curve), 0.5)
        }
        VisualProfileCurveFamily::CubicBezier => {
            let controls = [
                [-2.0, 0.1],
                [-0.7, -1.0 / 30.0],
                [0.7, -1.0 / 30.0],
                [2.0, 0.1],
            ]
            .map(|point| pair_point(document, "pair cubic control", frame, point));
            let curve = document
                .add_curve(&label, CurveDefinition::CubicBezier { controls })
                .unwrap();
            (curve, CurveSpan::line(curve), 0.5)
        }
        VisualProfileCurveFamily::ClampedBSpline => {
            let controls = [[-2.0, 0.1], [0.0, -0.1], [2.0, 0.1]]
                .map(|point| pair_point(document, "pair spline control", frame, point));
            let curve = document
                .add_curve(
                    &label,
                    CurveDefinition::BSpline {
                        form: DocumentBSplineForm::Clamped,
                        degree: 2,
                        controls: controls.to_vec(),
                        knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                        span_ids: vec![0],
                        next_span_id: 1,
                    },
                )
                .unwrap();
            (curve, CurveSpan::line(curve), 0.5)
        }
        VisualProfileCurveFamily::PeriodicBSpline => {
            let controls = [
                [-2.0, -1.0],
                [0.0, -2.0],
                [2.0, -1.0],
                [1.5, 1.5],
                [-1.5, 1.5],
            ]
            .map(|point| pair_point(document, "pair periodic control", frame, point));
            let curve = document
                .add_curve(
                    &label,
                    CurveDefinition::BSpline {
                        form: DocumentBSplineForm::Periodic,
                        degree: 2,
                        controls: controls.to_vec(),
                        knots: vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
                        span_ids: vec![0, 1, 2, 3, 4],
                        next_span_id: 5,
                    },
                )
                .unwrap();
            let target_span = CurveSpan { curve, segment: 0 };
            align_periodic_target(document, &controls, target_span, frame);
            (curve, target_span, 0.5)
        }
        VisualProfileCurveFamily::ClampedNurbs => {
            let controls = [[-2.0, 0.0], [0.0, 0.0], [2.0, 0.0]]
                .map(|point| pair_point(document, "pair NURBS control", frame, point));
            let weights = [1.0, 0.75, 1.0].map(|value| {
                scalar(
                    document,
                    "pair NURBS weight",
                    value,
                    ScalarUnit::Parameter,
                    ScalarDomain::Positive,
                )
            });
            let curve = document
                .add_curve(
                    &label,
                    CurveDefinition::Nurbs {
                        form: DocumentBSplineForm::Clamped,
                        degree: 2,
                        controls: controls.to_vec(),
                        weights: weights.to_vec(),
                        gauge_weight: weights[0],
                        knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                        span_ids: vec![0],
                        next_span_id: 1,
                    },
                )
                .unwrap();
            (curve, CurveSpan::line(curve), 0.5)
        }
        VisualProfileCurveFamily::PeriodicNurbs => {
            let controls = [
                [-2.0, -1.0],
                [0.0, -2.0],
                [2.0, -1.0],
                [1.5, 1.5],
                [-1.5, 1.5],
            ]
            .map(|point| pair_point(document, "pair periodic NURBS control", frame, point));
            let weights = [1.0, 0.999_999, 1.000_001, 0.999_998, 1.000_002].map(|value| {
                scalar(
                    document,
                    "pair periodic NURBS weight",
                    value,
                    ScalarUnit::Parameter,
                    ScalarDomain::Positive,
                )
            });
            let curve = document
                .add_curve(
                    &label,
                    CurveDefinition::Nurbs {
                        form: DocumentBSplineForm::Periodic,
                        degree: 1,
                        controls: controls.to_vec(),
                        weights: weights.to_vec(),
                        gauge_weight: weights[0],
                        knots: vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
                        span_ids: vec![0, 1, 2, 3, 4],
                        next_span_id: 5,
                    },
                )
                .unwrap();
            let target_span = CurveSpan { curve, segment: 0 };
            align_periodic_target(document, &controls, target_span, frame);
            (curve, target_span, 0.5)
        }
    };
    PairCurve {
        family,
        curve,
        target_span,
        target_parameter,
    }
}

fn finite_ordered(enclosure: [f64; 2]) -> bool {
    enclosure[0].is_finite() && enclosure[1].is_finite() && enclosure[0] <= enclosure[1]
}

fn assert_complete_pair_fixture(
    document: &SketchDocument,
    first: PairCurve,
    second: PairCurve,
    pair_label: &str,
) {
    let before = document.to_canonical_json().unwrap();
    let analysis = document.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(
        analysis.status,
        VisualProfileStatus::Complete,
        "{pair_label}: {analysis:#?}"
    );
    assert!(analysis.issues.is_empty(), "{pair_label}: {analysis:#?}");
    assert!(
        analysis.families.contains(&first.family),
        "{pair_label}: first role missing from {analysis:#?}"
    );
    assert!(
        analysis.families.contains(&second.family),
        "{pair_label}: second role missing from {analysis:#?}"
    );
    assert_ne!(
        first.curve, second.curve,
        "{pair_label}: fixtures must use two curves"
    );
    assert!(
        analysis.intersections.iter().all(|root| {
            finite_ordered(root.first_parameter_enclosure)
                && finite_ordered(root.second_parameter_enclosure)
                && root
                    .position_enclosure
                    .into_iter()
                    .flatten()
                    .all(f64::is_finite)
                && root.position_enclosure[0][0] <= root.position_enclosure[1][0]
                && root.position_enclosure[0][1] <= root.position_enclosure[1][1]
        }),
        "{pair_label}: non-finite or unordered root evidence: {analysis:#?}"
    );
    let known = analysis.intersections.iter().find(|root| {
        let direct = root.first_span == first.target_span
            && root.second_span == second.target_span
            && root.first_parameter_enclosure[0] <= first.target_parameter
            && first.target_parameter <= root.first_parameter_enclosure[1]
            && root.second_parameter_enclosure[0] <= second.target_parameter
            && second.target_parameter <= root.second_parameter_enclosure[1];
        let reverse = root.first_span == second.target_span
            && root.second_span == first.target_span
            && root.first_parameter_enclosure[0] <= second.target_parameter
            && second.target_parameter <= root.first_parameter_enclosure[1]
            && root.second_parameter_enclosure[0] <= first.target_parameter
            && first.target_parameter <= root.second_parameter_enclosure[1];
        direct || reverse
    });
    assert!(
        known.is_some(),
        "{pair_label}: known parameter root missing: {analysis:#?}"
    );
    let first_jet = document
        .evaluate_curve_jet(first.target_span, first.target_parameter)
        .unwrap();
    let second_jet = document
        .evaluate_curve_jet(second.target_span, second.target_parameter)
        .unwrap();
    for (role, jet) in [(first, &first_jet), (second, &second_jet)] {
        if matches!(
            role.family,
            VisualProfileCurveFamily::QuadraticBezier
                | VisualProfileCurveFamily::CubicBezier
                | VisualProfileCurveFamily::ClampedBSpline
        ) {
            assert!(
                jet.second_derivative.norm() > 1.0e-6,
                "{pair_label}: {:?} fixture is not genuinely curved",
                role.family
            );
        }
        if matches!(
            role.family,
            VisualProfileCurveFamily::ClampedNurbs | VisualProfileCurveFamily::PeriodicNurbs
        ) {
            let CurveDefinition::Nurbs { weights, .. } =
                &document.curve(role.curve).unwrap().definition
            else {
                unreachable!("NURBS family role must use NURBS geometry")
            };
            let values = weights
                .iter()
                .map(|weight| document.scalar(*weight).unwrap().value.to_bits())
                .collect::<std::collections::BTreeSet<_>>();
            assert!(
                values.len() > 1,
                "{pair_label}: {:?} must exercise the rational piece path",
                role.family
            );
        }
    }
    let cross = first_jet.first_derivative.x * second_jet.first_derivative.y
        - first_jet.first_derivative.y * second_jet.first_derivative.x;
    let scale = first_jet.first_derivative.norm() * second_jet.first_derivative.norm();
    assert!(
        cross.abs() > 0.25 * scale,
        "{pair_label}: known root is not transverse: cross={cross}, scale={scale}"
    );
    for contour in analysis.faces.iter().flat_map(|face| &face.contours) {
        assert!(contour.signed_area.is_finite(), "{pair_label}");
        assert!(contour.area_uncertainty.is_finite(), "{pair_label}");
        for edge in &contour.edges {
            assert!(edge.start.into_iter().all(f64::is_finite), "{pair_label}");
            assert!(edge.end.into_iter().all(f64::is_finite), "{pair_label}");
            assert!(
                edge.source_parameters.into_iter().all(f64::is_finite),
                "{pair_label}"
            );
            assert!(
                edge.source_parameter_enclosures
                    .into_iter()
                    .all(finite_ordered),
                "{pair_label}"
            );
        }
    }
    assert_eq!(
        document.to_canonical_json().unwrap(),
        before,
        "{pair_label}"
    );
}

fn add_circle_lens_pair(document: &mut SketchDocument) -> [PairCurve; 2] {
    [-1.0, 1.0].map(|x| {
        let center = document.add_point("lens center", [x, 0.0]).unwrap();
        let radius = scalar(
            document,
            "lens radius",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        );
        let curve = document
            .add_curve("lens circle", CurveDefinition::Circle { center, radius })
            .unwrap();
        PairCurve {
            family: VisualProfileCurveFamily::Circle,
            curve,
            target_span: CurveSpan::line(curve),
            target_parameter: if x < 0.0 {
                std::f64::consts::PI / 3.0
            } else {
                2.0 * std::f64::consts::PI / 3.0
            },
        }
    })
}

fn add_circle_arc_pair(document: &mut SketchDocument) -> [PairCurve; 2] {
    let circle_center = document
        .add_point("pair circle center", [0.0, 0.0])
        .unwrap();
    let circle_radius = scalar(
        document,
        "pair circle radius",
        2.0,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    );
    let circle = document
        .add_curve(
            "pair circle",
            CurveDefinition::Circle {
                center: circle_center,
                radius: circle_radius,
            },
        )
        .unwrap();
    let arc_center = document.add_point("pair arc center", [-1.0, 2.0]).unwrap();
    let arc_radius = scalar(
        document,
        "pair arc radius",
        1.0,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    );
    let start_angle = scalar(
        document,
        "pair arc start",
        -0.2,
        ScalarUnit::Angle,
        ScalarDomain::Finite,
    );
    let end_angle = scalar(
        document,
        "pair arc end",
        0.2,
        ScalarUnit::Angle,
        ScalarDomain::Finite,
    );
    let arc = document
        .add_curve(
            "pair arc",
            CurveDefinition::CircularArc {
                center: arc_center,
                radius: arc_radius,
                start_angle,
                end_angle,
                sweep: DocumentArcSweep::CounterClockwise,
            },
        )
        .unwrap();
    [
        PairCurve {
            family: VisualProfileCurveFamily::Circle,
            curve: circle,
            target_span: CurveSpan::line(circle),
            target_parameter: 0.5 * std::f64::consts::PI,
        },
        PairCurve {
            family: VisualProfileCurveFamily::CircularArc,
            curve: arc,
            target_span: CurveSpan::line(arc),
            target_parameter: 0.5,
        },
    ]
}

fn add_circle_ellipse_pair(document: &mut SketchDocument, arc: bool) -> [PairCurve; 2] {
    let circle_center = document
        .add_point("pair circle center", [0.0, 0.0])
        .unwrap();
    let circle_radius = scalar(
        document,
        "pair circle radius",
        2.0,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    );
    let circle = document
        .add_curve(
            "pair circle",
            CurveDefinition::Circle {
                center: circle_center,
                radius: circle_radius,
            },
        )
        .unwrap();
    let ellipse_parameter = if arc {
        0.0
    } else {
        20.0_f64.sqrt().atan2(7.0_f64.sqrt())
    };
    let ellipse_center = document
        .add_point(
            "pair ellipse center",
            if arc { [-1.0, 2.0] } else { [0.0, 0.0] },
        )
        .unwrap();
    let axis = document
        .add_point(
            "pair ellipse axis",
            if arc { [0.0, 2.0] } else { [3.0, 0.0] },
        )
        .unwrap();
    let ratio = pair_ratio(document, "pair ellipse ratio", if arc { 1.0 } else { 0.5 });
    let ellipse = if arc {
        let start_angle = scalar(
            document,
            "pair elliptical start",
            ellipse_parameter - 0.2,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        );
        let end_angle = scalar(
            document,
            "pair elliptical end",
            ellipse_parameter + 0.2,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        );
        document
            .add_curve(
                "pair elliptical arc",
                CurveDefinition::EllipticalArc {
                    center: ellipse_center,
                    major_axis_point: axis,
                    minor_axis_ratio: ratio,
                    start_angle,
                    end_angle,
                    sweep: DocumentArcSweep::CounterClockwise,
                },
            )
            .unwrap()
    } else {
        document
            .add_curve(
                "pair ellipse",
                CurveDefinition::Ellipse {
                    center: ellipse_center,
                    major_axis_point: axis,
                    minor_axis_ratio: ratio,
                },
            )
            .unwrap()
    };
    let position = document
        .evaluate_curve_jet(
            CurveSpan::line(ellipse),
            if arc { 0.5 } else { ellipse_parameter },
        )
        .unwrap()
        .position;
    [
        PairCurve {
            family: VisualProfileCurveFamily::Circle,
            curve: circle,
            target_span: CurveSpan::line(circle),
            target_parameter: position.y.atan2(position.x),
        },
        PairCurve {
            family: if arc {
                VisualProfileCurveFamily::EllipticalArc
            } else {
                VisualProfileCurveFamily::Ellipse
            },
            curve: ellipse,
            target_span: CurveSpan::line(ellipse),
            target_parameter: if arc { 0.5 } else { ellipse_parameter },
        },
    ]
}

#[test]
fn every_unordered_family_pair_isolated_by_named_roles() {
    let mut pair_count = 0_usize;
    let mut cross_family_count = 0_usize;
    let mut same_family_count = 0_usize;
    for (first_index, first_family) in ALL_PROFILE_FAMILIES.iter().copied().enumerate() {
        for (second_index, second_family) in ALL_PROFILE_FAMILIES
            .iter()
            .copied()
            .enumerate()
            .skip(first_index)
        {
            let pair_label = format!(
                "pair {pair_count}: {first_family:?} x {second_family:?} ({first_index}, {second_index})"
            );
            let model_scale = if first_family == VisualProfileCurveFamily::PeriodicNurbs
                && second_family == VisualProfileCurveFamily::PeriodicNurbs
            {
                10.0
            } else {
                2.0
            };
            let mut document = SketchDocument::new(model_scale).unwrap();
            let [first, second] = if first_family == VisualProfileCurveFamily::Circle
                && second_family == VisualProfileCurveFamily::Circle
            {
                add_circle_lens_pair(&mut document)
            } else if first_family == VisualProfileCurveFamily::Circle
                && second_family == VisualProfileCurveFamily::CircularArc
            {
                add_circle_arc_pair(&mut document)
            } else if first_family == VisualProfileCurveFamily::Circle
                && second_family == VisualProfileCurveFamily::Ellipse
            {
                add_circle_ellipse_pair(&mut document, false)
            } else if first_family == VisualProfileCurveFamily::Circle
                && second_family == VisualProfileCurveFamily::EllipticalArc
            {
                add_circle_ellipse_pair(&mut document, true)
            } else {
                let second_size = if first_family == VisualProfileCurveFamily::Circle {
                    0.25
                } else if first_family == VisualProfileCurveFamily::PeriodicNurbs
                    && second_family == VisualProfileCurveFamily::PeriodicNurbs
                {
                    2.0
                } else {
                    1.0
                };
                let first_size = if matches!(
                    (first_family, second_family),
                    (
                        VisualProfileCurveFamily::Line | VisualProfileCurveFamily::Polyline,
                        VisualProfileCurveFamily::PeriodicNurbs
                    )
                ) {
                    0.5
                } else {
                    2.0
                };
                let (second_tangent, second_normal) = if first_family
                    == VisualProfileCurveFamily::PeriodicNurbs
                    && second_family == VisualProfileCurveFamily::PeriodicNurbs
                {
                    ([0.0, 1.0], [-1.0, 0.0])
                } else {
                    ([0.6, 0.8], [-0.8, 0.6])
                };
                [
                    add_pair_curve(
                        &mut document,
                        first_family,
                        PairFrame::axes([0.0, 2.0], [-1.0, 0.0], [0.0, -1.0], first_size),
                        first_index,
                    ),
                    add_pair_curve(
                        &mut document,
                        second_family,
                        PairFrame::axes([0.0, 2.0], second_tangent, second_normal, second_size),
                        second_index,
                    ),
                ]
            };
            assert_complete_pair_fixture(&document, first, second, &pair_label);
            if first_family == VisualProfileCurveFamily::Circle {
                let analysis = document.analyze_visual_profiles(VisualProfileOptions::default());
                let expected = match second_family {
                    VisualProfileCurveFamily::Circle => Some(2),
                    VisualProfileCurveFamily::CircularArc
                    | VisualProfileCurveFamily::EllipticalArc => Some(1),
                    VisualProfileCurveFamily::Ellipse => Some(4),
                    _ => None,
                };
                if let Some(expected) = expected {
                    assert_eq!(
                        analysis.intersections.len(),
                        expected,
                        "{pair_label}: canonical circular oracle: {analysis:#?}"
                    );
                }
            }
            pair_count += 1;
            if first_family == second_family {
                same_family_count += 1;
            } else {
                cross_family_count += 1;
            }
        }
    }
    assert_eq!(pair_count, 120);
    assert_eq!(cross_family_count, 105);
    assert_eq!(same_family_count, 15);
}

#[derive(Clone, Copy)]
enum SelfIntersectionExpectation {
    MathematicallyImpossible,
    Loop,
}

const SELF_INTERSECTION_FAMILIES: [(VisualProfileCurveFamily, SelfIntersectionExpectation); 6] = [
    (
        VisualProfileCurveFamily::QuadraticBezier,
        SelfIntersectionExpectation::MathematicallyImpossible,
    ),
    (
        VisualProfileCurveFamily::CubicBezier,
        SelfIntersectionExpectation::Loop,
    ),
    (
        VisualProfileCurveFamily::ClampedBSpline,
        SelfIntersectionExpectation::Loop,
    ),
    (
        VisualProfileCurveFamily::PeriodicBSpline,
        SelfIntersectionExpectation::Loop,
    ),
    (
        VisualProfileCurveFamily::ClampedNurbs,
        SelfIntersectionExpectation::Loop,
    ),
    (
        VisualProfileCurveFamily::PeriodicNurbs,
        SelfIntersectionExpectation::Loop,
    ),
];

fn add_self_intersection_curve(
    document: &mut SketchDocument,
    family: VisualProfileCurveFamily,
) -> geosolve_sketch::CurveId {
    let loop_controls = [[0.0, 0.0], [2.0, 3.0], [-2.0, 3.0], [84.0 / 79.0, 0.0]];
    match family {
        VisualProfileCurveFamily::QuadraticBezier => {
            let controls = [[-2.0, 1.0], [0.0, -2.0], [2.0, 1.0]]
                .map(|point| document.add_point("non-loop quadratic", point).unwrap());
            document
                .add_curve(
                    "quadratic cannot self intersect",
                    CurveDefinition::QuadraticBezier { controls },
                )
                .unwrap()
        }
        VisualProfileCurveFamily::CubicBezier => {
            let controls =
                loop_controls.map(|point| document.add_point("cubic loop", point).unwrap());
            document
                .add_curve("cubic loop", CurveDefinition::CubicBezier { controls })
                .unwrap()
        }
        VisualProfileCurveFamily::ClampedBSpline => {
            let controls = loop_controls
                .map(|point| document.add_point("clamped spline loop", point).unwrap());
            document
                .add_curve(
                    "clamped spline loop",
                    CurveDefinition::BSpline {
                        form: DocumentBSplineForm::Clamped,
                        degree: 3,
                        controls: controls.to_vec(),
                        knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
                        span_ids: vec![0],
                        next_span_id: 1,
                    },
                )
                .unwrap()
        }
        VisualProfileCurveFamily::PeriodicBSpline => {
            let controls = [[-2.0, -1.0], [2.0, 1.0], [-2.0, 1.0], [2.0, -1.0]]
                .map(|point| document.add_point("periodic spline loop", point).unwrap());
            document
                .add_curve(
                    "periodic spline loop",
                    CurveDefinition::BSpline {
                        form: DocumentBSplineForm::Periodic,
                        degree: 1,
                        controls: controls.to_vec(),
                        knots: vec![0.0, 1.0, 2.0, 3.0, 4.0],
                        span_ids: vec![0, 1, 2, 3],
                        next_span_id: 4,
                    },
                )
                .unwrap()
        }
        VisualProfileCurveFamily::ClampedNurbs => {
            let controls =
                loop_controls.map(|point| document.add_point("clamped NURBS loop", point).unwrap());
            let weights = [1.0, 0.9, 1.1, 0.95].map(|value| {
                scalar(
                    document,
                    "clamped loop weight",
                    value,
                    ScalarUnit::Parameter,
                    ScalarDomain::Positive,
                )
            });
            document
                .add_curve(
                    "clamped NURBS loop",
                    CurveDefinition::Nurbs {
                        form: DocumentBSplineForm::Clamped,
                        degree: 3,
                        controls: controls.to_vec(),
                        weights: weights.to_vec(),
                        gauge_weight: weights[0],
                        knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
                        span_ids: vec![0],
                        next_span_id: 1,
                    },
                )
                .unwrap()
        }
        VisualProfileCurveFamily::PeriodicNurbs => {
            let controls = [[-2.0, -1.0], [2.0, 1.0], [-2.0, 1.0], [2.0, -1.0]]
                .map(|point| document.add_point("periodic NURBS loop", point).unwrap());
            let weights = [1.0, 0.8, 1.2, 0.9].map(|value| {
                scalar(
                    document,
                    "periodic loop weight",
                    value,
                    ScalarUnit::Parameter,
                    ScalarDomain::Positive,
                )
            });
            document
                .add_curve(
                    "periodic NURBS loop",
                    CurveDefinition::Nurbs {
                        form: DocumentBSplineForm::Periodic,
                        degree: 1,
                        controls: controls.to_vec(),
                        weights: weights.to_vec(),
                        gauge_weight: weights[0],
                        knots: vec![0.0, 1.0, 2.0, 3.0, 4.0],
                        span_ids: vec![0, 1, 2, 3],
                        next_span_id: 4,
                    },
                )
                .unwrap()
        }
        _ => unreachable!("self-intersection table contains only implementation roles"),
    }
}

#[test]
fn every_eligible_family_has_truthful_self_intersection_evidence() {
    let mut impossible_count = 0_usize;
    let mut loop_count = 0_usize;
    for (family, expectation) in SELF_INTERSECTION_FAMILIES {
        let mut document = SketchDocument::new(3.0).unwrap();
        let curve = add_self_intersection_curve(&mut document, family);
        let before = document.to_canonical_json().unwrap();
        let analysis = document.analyze_visual_profiles(VisualProfileOptions::default());
        assert_eq!(document.to_canonical_json().unwrap(), before, "{family:?}");
        assert!(
            analysis.families.contains(&family),
            "{family:?}: {analysis:#?}"
        );
        match expectation {
            SelfIntersectionExpectation::MathematicallyImpossible => {
                // A regular polynomial quadratic is injective unless it retraces or degenerates;
                // neither case is an eligible transverse self-root.
                impossible_count += 1;
                assert_eq!(
                    analysis.status,
                    VisualProfileStatus::Complete,
                    "{analysis:#?}"
                );
                assert!(analysis.intersections.is_empty(), "{analysis:#?}");
                assert!(analysis.faces.is_empty(), "{analysis:#?}");
            }
            SelfIntersectionExpectation::Loop => {
                loop_count += 1;
                assert_eq!(
                    analysis.status,
                    VisualProfileStatus::Complete,
                    "{family:?}: {analysis:#?}"
                );
                let roots = analysis
                    .intersections
                    .iter()
                    .filter(|root| {
                        root.first_span.curve == curve && root.second_span.curve == curve
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    roots.len(),
                    1,
                    "{family:?}: canonical loop oracle: {analysis:#?}"
                );
                assert!(
                    roots.iter().all(|root| {
                        finite_ordered(root.first_parameter_enclosure)
                            && finite_ordered(root.second_parameter_enclosure)
                            && (root.first_span != root.second_span
                                || root.first_parameter_enclosure[1]
                                    < root.second_parameter_enclosure[0]
                                || root.second_parameter_enclosure[1]
                                    < root.first_parameter_enclosure[0])
                    }),
                    "{family:?}: {analysis:#?}"
                );
                assert!(!analysis.faces.is_empty(), "{family:?}: {analysis:#?}");
            }
        }
    }
    assert_eq!(impossible_count, 1);
    assert_eq!(loop_count, 5);
}

fn exact_self_intersecting_nurbs(
    document: &mut SketchDocument,
    controls: [[f64; 2]; 4],
    weights: [f64; 4],
) -> geosolve_sketch::CurveId {
    let controls = controls.map(|position| {
        document
            .add_point("exact NURBS loop control", position)
            .unwrap()
    });
    let weights = weights.map(|value| {
        scalar(
            document,
            "exact NURBS loop weight",
            value,
            ScalarUnit::Parameter,
            ScalarDomain::Positive,
        )
    });
    document
        .add_curve(
            "exact NURBS loop",
            CurveDefinition::Nurbs {
                form: DocumentBSplineForm::Clamped,
                degree: 3,
                controls: controls.to_vec(),
                weights: weights.to_vec(),
                gauge_weight: weights[0],
                knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
                span_ids: vec![0],
                next_span_id: 1,
            },
        )
        .unwrap()
}

fn capsule_local_support_nurbs(control_four: [f64; 2]) -> SketchDocument {
    let mut document = SketchDocument::new(10.0).unwrap();
    let controls = [
        [-4.0, 0.0],
        [0.748_607_242_339_833_2, 1.759_155_630_168_501],
        [-3.547_595_806_693_728, -0.959_131_909_241_292_9],
        control_four,
        [-5.971_448_467_966_574, 0.651_899_521_233_600_6],
        [4.0, 0.5],
    ]
    .map(|position| {
        document
            .add_point("capsule NURBS control", position)
            .unwrap()
    });
    let weights = [0.8, 1.0, 1.3, 0.7, 1.15, 0.9].map(|value| {
        scalar(
            &mut document,
            "capsule NURBS weight",
            value,
            ScalarUnit::Parameter,
            ScalarDomain::Positive,
        )
    });
    document
        .add_curve(
            "capsule local-support NURBS",
            CurveDefinition::Nurbs {
                form: DocumentBSplineForm::Clamped,
                degree: 3,
                controls: controls.to_vec(),
                weights: weights.to_vec(),
                gauge_weight: weights[1],
                knots: vec![0.0, 0.0, 0.0, 0.0, 0.34, 0.67, 1.0, 1.0, 1.0, 1.0],
                span_ids: vec![41, 73, 89],
                next_span_id: 90,
            },
        )
        .unwrap();
    document
}

#[test]
fn capsule_nurbs_perturbation_preserves_self_intersection_faces() {
    let first: [f64; 2] = [1.145_186_802_651_284, -0.073_590_091_813_208_14];
    let second: [f64; 2] = [0.615_713_999_251_247_5, -0.290_589_840_851_838_7];
    let analyses = (0..=4)
        .map(|step| {
            let fraction = f64::from(step) / 4.0;
            let control = [
                (second[0] - first[0]).mul_add(fraction, first[0]),
                (second[1] - first[1]).mul_add(fraction, first[1]),
            ];
            capsule_local_support_nurbs(control)
                .analyze_visual_profiles(VisualProfileOptions::default())
        })
        .collect::<Vec<_>>();
    for (step, analysis) in analyses.iter().enumerate() {
        assert_eq!(
            analysis.status,
            VisualProfileStatus::Complete,
            "step={step}: {analysis:#?}"
        );
        assert!(analysis.issues.is_empty(), "step={step}: {analysis:#?}");
        assert_eq!(
            analysis.intersections.len(),
            4,
            "step={step}: {analysis:#?}"
        );
        assert_eq!(analysis.faces.len(), 4, "step={step}: {analysis:#?}");
        assert_eq!(analysis.fragment_count, 11, "step={step}: {analysis:#?}");
    }
    let topology = |analysis: &geosolve_sketch::VisualProfileAnalysis| {
        analysis
            .intersections
            .iter()
            .map(|root| (root.first_span.segment, root.second_span.segment))
            .collect::<Vec<_>>()
    };
    assert_eq!(topology(&analyses[0]), topology(&analyses[4]));
    let face_edges = |analysis: &geosolve_sketch::VisualProfileAnalysis| {
        let mut counts = analysis
            .faces
            .iter()
            .map(|face| {
                face.contours
                    .iter()
                    .map(|contour| contour.edges.len())
                    .sum()
            })
            .collect::<Vec<usize>>();
        counts.sort_unstable();
        counts
    };
    assert_eq!(face_edges(&analyses[0]), face_edges(&analyses[4]));
}

#[test]
fn nurbs_self_root_on_recursive_partition_boundary_is_certified() {
    let mut document = SketchDocument::new(40.0).unwrap();
    let curve = exact_self_intersecting_nurbs(
        &mut document,
        [[12.0, -9.0], [-6.0, 6.5], [-4.0, -13.0], [18.0, 4.5]],
        [1.0, 2.0, 1.0, 2.0],
    );
    let analysis = document.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(
        analysis.status,
        VisualProfileStatus::Complete,
        "{analysis:#?}"
    );
    let roots = analysis
        .intersections
        .iter()
        .filter(|root| root.first_span.curve == curve && root.second_span.curve == curve)
        .collect::<Vec<_>>();
    assert_eq!(roots.len(), 1, "{analysis:#?}");
    assert!(roots[0].first_parameter_enclosure[0] <= 0.25);
    assert!(roots[0].first_parameter_enclosure[1] >= 0.25);
    assert!(roots[0].second_parameter_enclosure[0] <= 0.5);
    assert!(roots[0].second_parameter_enclosure[1] >= 0.5);
    assert!(!analysis.faces.is_empty(), "{analysis:#?}");
}

#[test]
fn nurbs_partition_boundary_self_root_is_similarity_invariant() {
    let base = [[12.0, -9.0], [-6.0, 6.5], [-4.0, -13.0], [18.0, 4.5]];
    let angle = 0.37_f64;
    for scale in [1.0e-6, 1.0, 1.0e6] {
        for reflection in [-1.0, 1.0] {
            let controls = base.map(|point| {
                let reflected = [point[0], reflection * point[1]];
                [
                    scale * (angle.cos() * reflected[0] - angle.sin() * reflected[1] + 23.0),
                    scale * (angle.sin() * reflected[0] + angle.cos() * reflected[1] - 17.0),
                ]
            });
            let mut document = SketchDocument::new(200.0 * scale).unwrap();
            let curve =
                exact_self_intersecting_nurbs(&mut document, controls, [1.0, 2.0, 1.0, 2.0]);
            let analysis = document.analyze_visual_profiles(VisualProfileOptions::default());
            assert_eq!(
                analysis.status,
                VisualProfileStatus::Complete,
                "scale={scale:e}, reflection={reflection}: {analysis:#?}"
            );
            assert_eq!(
                analysis
                    .intersections
                    .iter()
                    .filter(|root| {
                        root.first_span.curve == curve && root.second_span.curve == curve
                    })
                    .count(),
                1,
                "scale={scale:e}, reflection={reflection}: {analysis:#?}"
            );
            assert!(!analysis.faces.is_empty(), "{analysis:#?}");
        }
    }
}

#[test]
fn nurbs_self_root_survives_geometry_preserving_knot_insertion_away_from_root() {
    let mut document = SketchDocument::new(160.0).unwrap();
    let curve = exact_self_intersecting_nurbs(
        &mut document,
        [[90.0, -45.0], [-19.0, 24.5], [-38.0, -49.0], [45.0, 22.5]],
        [1.0, 2.0, 1.0, 2.0],
    );
    let before_samples = (0..=16)
        .map(|sample| {
            let parameter = f64::from(sample) / 16.0;
            document
                .evaluate_curve_jet(CurveSpan::line(curve), parameter)
                .unwrap()
                .position
        })
        .collect::<Vec<_>>();
    let before = document.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(before.status, VisualProfileStatus::Complete, "{before:#?}");
    assert_eq!(before.intersections.len(), 1, "{before:#?}");

    let inserted = 1.0 / 5.0;
    document.insert_nurbs_knot(curve, inserted).unwrap();
    let spans = document.curve_spans(curve).unwrap();
    assert_eq!(spans.len(), 2);
    for (sample, expected) in (0..=16).zip(before_samples) {
        let global = f64::from(sample) / 16.0;
        let (span, parameter) = if global <= inserted {
            (spans[0], global / inserted)
        } else {
            (spans[1], (global - inserted) / (1.0 - inserted))
        };
        let actual = document
            .evaluate_curve_jet(span, parameter)
            .unwrap()
            .position;
        assert!((actual.x - expected.x).abs() <= 1.0e-10);
        assert!((actual.y - expected.y).abs() <= 1.0e-10);
    }

    let after = document.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(after.status, VisualProfileStatus::Complete, "{after:#?}");
    let roots = after
        .intersections
        .iter()
        .filter(|root| root.first_span.curve == curve && root.second_span.curve == curve)
        .collect::<Vec<_>>();
    assert_eq!(roots.len(), 1, "{after:#?}");
    assert!(!after.faces.is_empty(), "{after:#?}");
}

#[test]
fn nurbs_self_root_on_inserted_semantic_boundary_fails_closed() {
    let mut document = SketchDocument::new(160.0).unwrap();
    let curve = exact_self_intersecting_nurbs(
        &mut document,
        [[90.0, -45.0], [-19.0, 24.5], [-38.0, -49.0], [45.0, 22.5]],
        [1.0, 2.0, 1.0, 2.0],
    );
    document.insert_nurbs_knot(curve, 3.0 / 8.0).unwrap();
    let analysis = document.analyze_visual_profiles(VisualProfileOptions::default());
    assert_ne!(
        analysis.status,
        VisualProfileStatus::Complete,
        "{analysis:#?}"
    );
    assert!(analysis.faces.is_empty(), "{analysis:#?}");
    assert!(analysis.issues.iter().any(|issue| matches!(
        issue.kind,
        VisualProfileIssueKind::UnresolvedIntersection { first, second }
            if first.curve == curve && second.curve == curve
    )));
}

fn assert_one_complete_face(document: &SketchDocument, family: VisualProfileCurveFamily) -> f64 {
    let analysis = document.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(
        analysis.scope,
        VisualProfileGeometryScope::AllBuiltInPlanarCurves
    );
    assert_eq!(
        analysis.status,
        VisualProfileStatus::Complete,
        "{analysis:#?}"
    );
    assert!(analysis.issues.is_empty(), "{analysis:#?}");
    assert!(analysis.families.contains(&family));
    assert_eq!(analysis.faces.len(), 1, "{analysis:#?}");
    assert!(analysis.faces[0].visual_area > 0.0);
    assert!(analysis.faces[0].area_uncertainty.is_finite());
    analysis.faces[0].visual_area
}

#[test]
fn standalone_circle_and_ellipse_publish_complete_disks() {
    let mut circle = SketchDocument::new(2.0).unwrap();
    let center = circle.add_point("center", [0.0, 0.0]).unwrap();
    let radius = scalar(
        &mut circle,
        "radius",
        2.0,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    );
    circle
        .add_curve("circle", CurveDefinition::Circle { center, radius })
        .unwrap();
    let area = assert_one_complete_face(&circle, VisualProfileCurveFamily::Circle);
    assert!((area - 4.0 * std::f64::consts::PI).abs() <= 1.0e-12);

    let mut ellipse = SketchDocument::new(3.0).unwrap();
    let center = ellipse.add_point("center", [1.0, -2.0]).unwrap();
    let axis = ellipse.add_point("major", [4.0, -2.0]).unwrap();
    let ratio = scalar(
        &mut ellipse,
        "ratio",
        0.5,
        ScalarUnit::Parameter,
        ScalarDomain::Bounded {
            lower: f64::from_bits(1),
            upper: 1.0,
        },
    );
    ellipse
        .add_curve(
            "ellipse",
            CurveDefinition::Ellipse {
                center,
                major_axis_point: axis,
                minor_axis_ratio: ratio,
            },
        )
        .unwrap();
    let area = assert_one_complete_face(&ellipse, VisualProfileCurveFamily::Ellipse);
    assert!((area - 4.5 * std::f64::consts::PI).abs() <= 1.0e-11);
}

#[test]
fn circular_roots_are_certified_at_large_positive_and_negative_windings() {
    for winding in [-100.0_f64, 100.0] {
        let mut document = SketchDocument::new(2.0).unwrap();
        let arc_center = document.add_point("wound arc center", [0.0, 0.0]).unwrap();
        let arc_radius = scalar(
            &mut document,
            "wound arc radius",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        );
        let angle_offset = winding * std::f64::consts::TAU;
        let start = scalar(
            &mut document,
            "wound arc start",
            angle_offset - std::f64::consts::FRAC_PI_2,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        );
        let end = scalar(
            &mut document,
            "wound arc end",
            angle_offset + std::f64::consts::FRAC_PI_2,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        );
        let arc = document
            .add_curve(
                "large-winding arc",
                CurveDefinition::CircularArc {
                    center: arc_center,
                    radius: arc_radius,
                    start_angle: start,
                    end_angle: end,
                    sweep: DocumentArcSweep::CounterClockwise,
                },
            )
            .unwrap();
        let circle_center = document.add_point("circle center", [2.0, 0.0]).unwrap();
        let circle_radius = scalar(
            &mut document,
            "circle radius",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        );
        let circle = document
            .add_curve(
                "intersecting circle",
                CurveDefinition::Circle {
                    center: circle_center,
                    radius: circle_radius,
                },
            )
            .unwrap();

        let analysis = document.analyze_visual_profiles(VisualProfileOptions::default());
        assert_eq!(
            analysis.status,
            VisualProfileStatus::Complete,
            "winding={winding}: {analysis:#?}"
        );
        let roots = analysis
            .intersections
            .iter()
            .filter(|root| {
                [root.first_span.curve, root.second_span.curve].contains(&arc)
                    && [root.first_span.curve, root.second_span.curve].contains(&circle)
            })
            .count();
        assert_eq!(roots, 2, "winding={winding}: {analysis:#?}");
    }
}

#[test]
fn polynomial_and_rational_contours_publish_complete_faces() {
    let mut bezier = SketchDocument::new(3.0).unwrap();
    let controls = [[-2.0, 0.0], [-1.0, 3.0], [1.0, 3.0], [2.0, 0.0]]
        .map(|point| bezier.add_point("control", point).unwrap());
    bezier
        .add_curve("cubic", CurveDefinition::CubicBezier { controls })
        .unwrap();
    line(&mut bezier, "close", controls[3], controls[0]);
    assert_one_complete_face(&bezier, VisualProfileCurveFamily::CubicBezier);

    let mut rational = SketchDocument::new(2.0).unwrap();
    let start = rational.add_point("start", [-2.0, 0.0]).unwrap();
    let end = rational.add_point("end", [2.0, 0.0]).unwrap();
    let weight = scalar(
        &mut rational,
        "weight",
        1.0,
        ScalarUnit::Parameter,
        ScalarDomain::Bounded {
            lower: geosolve_sketch::MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
            upper: f64::MAX,
        },
    );
    rational
        .add_curve(
            "rational conic",
            CurveDefinition::RationalQuadraticConic {
                start,
                weighted_middle: [0.0, 3.0],
                middle_weight: weight,
                end,
            },
        )
        .unwrap();
    line(&mut rational, "close", end, start);
    assert_one_complete_face(&rational, VisualProfileCurveFamily::RationalQuadraticConic);
}

#[test]
fn circular_elliptical_and_analytic_conic_caps_are_complete() {
    let mut circular = SketchDocument::new(2.0).unwrap();
    let center = circular.add_point("center", [0.0, 0.0]).unwrap();
    let radius = scalar(
        &mut circular,
        "radius",
        2.0,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    );
    let start = scalar(
        &mut circular,
        "start",
        0.0,
        ScalarUnit::Angle,
        ScalarDomain::Finite,
    );
    let end = scalar(
        &mut circular,
        "end",
        std::f64::consts::PI,
        ScalarUnit::Angle,
        ScalarDomain::Finite,
    );
    let arc = circular
        .add_curve(
            "arc",
            CurveDefinition::CircularArc {
                center,
                radius,
                start_angle: start,
                end_angle: end,
                sweep: DocumentArcSweep::CounterClockwise,
            },
        )
        .unwrap();
    close_derived_curve(&mut circular, arc);
    let area = assert_one_complete_face(&circular, VisualProfileCurveFamily::CircularArc);
    assert!((area - 2.0 * std::f64::consts::PI).abs() <= 1.0e-10);

    let mut elliptical = SketchDocument::new(3.0).unwrap();
    let center = elliptical.add_point("center", [0.0, 0.0]).unwrap();
    let axis = elliptical.add_point("axis", [3.0, 0.0]).unwrap();
    let ratio = scalar(
        &mut elliptical,
        "ratio",
        0.5,
        ScalarUnit::Parameter,
        ScalarDomain::Bounded {
            lower: f64::from_bits(1),
            upper: 1.0,
        },
    );
    let start = scalar(
        &mut elliptical,
        "start",
        0.0,
        ScalarUnit::Angle,
        ScalarDomain::Finite,
    );
    let end = scalar(
        &mut elliptical,
        "end",
        std::f64::consts::PI,
        ScalarUnit::Angle,
        ScalarDomain::Finite,
    );
    let arc = elliptical
        .add_curve(
            "elliptical arc",
            CurveDefinition::EllipticalArc {
                center,
                major_axis_point: axis,
                minor_axis_ratio: ratio,
                start_angle: start,
                end_angle: end,
                sweep: DocumentArcSweep::CounterClockwise,
            },
        )
        .unwrap();
    close_derived_curve(&mut elliptical, arc);
    assert_one_complete_face(&elliptical, VisualProfileCurveFamily::EllipticalArc);

    let mut parabola = SketchDocument::new(2.0).unwrap();
    let vertex = parabola.add_point("vertex", [0.0, 0.0]).unwrap();
    let focus = parabola.add_point("focus", [0.0, 0.5]).unwrap();
    let trim_start = scalar(
        &mut parabola,
        "trim start",
        -2.0,
        ScalarUnit::Parameter,
        ScalarDomain::Finite,
    );
    let trim_end = scalar(
        &mut parabola,
        "trim end",
        2.0,
        ScalarUnit::Parameter,
        ScalarDomain::Finite,
    );
    let curve = parabola
        .add_curve(
            "parabola",
            CurveDefinition::ParabolaSegment {
                vertex,
                focus,
                trim_start,
                trim_end,
            },
        )
        .unwrap();
    close_derived_curve(&mut parabola, curve);
    assert_one_complete_face(&parabola, VisualProfileCurveFamily::Parabola);

    let mut hyperbola = SketchDocument::new(2.0).unwrap();
    let center = hyperbola.add_point("center", [0.0, 0.0]).unwrap();
    let axis = hyperbola.add_point("axis", [1.0, 0.0]).unwrap();
    let conjugate = scalar(
        &mut hyperbola,
        "conjugate",
        1.0,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    );
    let trim_start = scalar(
        &mut hyperbola,
        "trim start",
        -1.0,
        ScalarUnit::Parameter,
        ScalarDomain::Finite,
    );
    let trim_end = scalar(
        &mut hyperbola,
        "trim end",
        1.0,
        ScalarUnit::Parameter,
        ScalarDomain::Finite,
    );
    let curve = hyperbola
        .add_curve(
            "hyperbola",
            CurveDefinition::HyperbolaSegment {
                center,
                transverse_axis_point: axis,
                semi_conjugate: conjugate,
                branch: DocumentHyperbolaBranch::Positive,
                trim_start,
                trim_end,
            },
        )
        .unwrap();
    close_derived_curve(&mut hyperbola, curve);
    assert_one_complete_face(&hyperbola, VisualProfileCurveFamily::Hyperbola);
}

#[test]
fn two_circle_lens_nested_curves_and_transverse_roots_are_bounded() {
    let mut lens = SketchDocument::new(2.0).unwrap();
    for (label, x) in [("first", -1.0), ("second", 1.0)] {
        let center = lens.add_point(label, [x, 0.0]).unwrap();
        let radius = scalar(
            &mut lens,
            &format!("{label} radius"),
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        );
        lens.add_curve(label, CurveDefinition::Circle { center, radius })
            .unwrap();
    }
    let analysis = lens.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(
        analysis.status,
        VisualProfileStatus::Complete,
        "{analysis:#?}"
    );
    assert_eq!(analysis.intersections.len(), 2, "{analysis:#?}");
    assert!(analysis.faces.len() >= 3, "{analysis:#?}");
    assert!(analysis.intersections.iter().all(|root| {
        root.first_parameter_enclosure[0] <= root.first_parameter_enclosure[1]
            && root.second_parameter_enclosure[0] <= root.second_parameter_enclosure[1]
    }));

    let mut nested = SketchDocument::new(4.0).unwrap();
    let center = nested.add_point("center", [0.0, 0.0]).unwrap();
    let outer_radius = scalar(
        &mut nested,
        "outer radius",
        4.0,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    );
    nested
        .add_curve(
            "outer",
            CurveDefinition::Circle {
                center,
                radius: outer_radius,
            },
        )
        .unwrap();
    let axis = nested.add_point("inner axis", [2.0, 0.0]).unwrap();
    let ratio = scalar(
        &mut nested,
        "inner ratio",
        0.5,
        ScalarUnit::Parameter,
        ScalarDomain::Bounded {
            lower: f64::from_bits(1),
            upper: 1.0,
        },
    );
    nested
        .add_curve(
            "inner",
            CurveDefinition::Ellipse {
                center,
                major_axis_point: axis,
                minor_axis_ratio: ratio,
            },
        )
        .unwrap();
    let analysis = nested.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(
        analysis.status,
        VisualProfileStatus::Complete,
        "{analysis:#?}"
    );
    assert_eq!(analysis.faces.len(), 2, "{analysis:#?}");
    assert!(analysis.faces.iter().any(|face| face.contours.len() == 2));

    let mut transverse = SketchDocument::new(3.0).unwrap();
    let center = transverse.add_point("center", [0.0, 0.0]).unwrap();
    let axis = transverse.add_point("axis", [3.0, 0.0]).unwrap();
    let ratio = scalar(
        &mut transverse,
        "ratio",
        0.5,
        ScalarUnit::Parameter,
        ScalarDomain::Bounded {
            lower: f64::from_bits(1),
            upper: 1.0,
        },
    );
    transverse
        .add_curve(
            "ellipse",
            CurveDefinition::Ellipse {
                center,
                major_axis_point: axis,
                minor_axis_ratio: ratio,
            },
        )
        .unwrap();
    let a = transverse.add_point("a", [-4.0, 0.5]).unwrap();
    let b = transverse.add_point("b", [4.0, 0.5]).unwrap();
    line(&mut transverse, "line", a, b);
    let analysis = transverse.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(
        analysis.status,
        VisualProfileStatus::Complete,
        "{analysis:#?}"
    );
    assert_eq!(analysis.intersections.len(), 2, "{analysis:#?}");
}

#[test]
fn clamped_spline_rational_nurbs_and_cubic_self_intersection_are_complete() {
    let controls = [[-2.0, 0.0], [0.0, 3.0], [2.0, 0.0]];
    let mut spline = SketchDocument::new(2.0).unwrap();
    let points = controls.map(|point| spline.add_point("control", point).unwrap());
    spline
        .add_curve(
            "spline",
            CurveDefinition::BSpline {
                form: DocumentBSplineForm::Clamped,
                degree: 2,
                controls: points.to_vec(),
                knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                span_ids: vec![0],
                next_span_id: 1,
            },
        )
        .unwrap();
    line(&mut spline, "close", points[2], points[0]);
    assert_one_complete_face(&spline, VisualProfileCurveFamily::ClampedBSpline);

    let mut nurbs = SketchDocument::new(2.0).unwrap();
    let points = controls.map(|point| nurbs.add_point("control", point).unwrap());
    let weights = [1.0, 0.7, 1.0].map(|value| {
        scalar(
            &mut nurbs,
            "weight",
            value,
            ScalarUnit::Parameter,
            ScalarDomain::Positive,
        )
    });
    nurbs
        .add_curve(
            "NURBS",
            CurveDefinition::Nurbs {
                form: DocumentBSplineForm::Clamped,
                degree: 2,
                controls: points.to_vec(),
                weights: weights.to_vec(),
                gauge_weight: weights[0],
                knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                span_ids: vec![0],
                next_span_id: 1,
            },
        )
        .unwrap();
    line(&mut nurbs, "close", points[2], points[0]);
    assert_one_complete_face(&nurbs, VisualProfileCurveFamily::ClampedNurbs);

    let mut self_crossing = SketchDocument::new(3.0).unwrap();
    let controls = [[0.0, 0.0], [2.0, 3.0], [-2.0, 3.0], [84.0 / 79.0, 0.0]]
        .map(|point| self_crossing.add_point("control", point).unwrap());
    self_crossing
        .add_curve("self crossing", CurveDefinition::CubicBezier { controls })
        .unwrap();
    let analysis = self_crossing.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(
        analysis.status,
        VisualProfileStatus::Complete,
        "{analysis:#?}"
    );
    assert_eq!(analysis.intersections.len(), 1, "{analysis:#?}");
    assert_eq!(analysis.faces.len(), 1, "{analysis:#?}");
}

#[test]
fn adjacent_spline_spans_are_checked_beyond_their_shared_boundary() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let controls = [[1.0, -1.0], [0.0, 1.0], [0.0, -1.0], [1.0, 1.0]]
        .map(|point| document.add_point("control", point).unwrap());
    document
        .add_curve(
            "crossing adjacent spans",
            CurveDefinition::BSpline {
                form: DocumentBSplineForm::Clamped,
                degree: 2,
                controls: controls.to_vec(),
                knots: vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0],
                span_ids: vec![0, 1],
                next_span_id: 2,
            },
        )
        .unwrap();

    let analysis = document.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(
        analysis.status,
        VisualProfileStatus::Complete,
        "{analysis:#?}"
    );
    assert_eq!(analysis.intersections.len(), 1, "{analysis:#?}");
    assert_eq!(analysis.faces.len(), 1, "{analysis:#?}");
}

#[test]
fn periodic_bspline_and_nurbs_publish_complete_contours() {
    let controls = [
        [-2.0, -1.0],
        [0.0, -2.0],
        [2.0, -1.0],
        [1.5, 1.5],
        [-1.5, 1.5],
    ];
    let mut spline = SketchDocument::new(2.0).unwrap();
    let spline_controls = controls.map(|point| spline.add_point("control", point).unwrap());
    spline
        .add_curve(
            "periodic spline",
            CurveDefinition::BSpline {
                form: DocumentBSplineForm::Periodic,
                degree: 2,
                controls: spline_controls.to_vec(),
                knots: vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
                span_ids: vec![0, 1, 2, 3, 4],
                next_span_id: 5,
            },
        )
        .unwrap();
    assert_one_complete_face(&spline, VisualProfileCurveFamily::PeriodicBSpline);

    let mut nurbs = SketchDocument::new(2.0).unwrap();
    let nurbs_controls = controls.map(|point| nurbs.add_point("control", point).unwrap());
    let weights = [1.0, 0.8, 1.2, 0.9, 1.1]
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            scalar(
                &mut nurbs,
                &format!("weight {index}"),
                value,
                ScalarUnit::Parameter,
                ScalarDomain::Positive,
            )
        })
        .collect::<Vec<_>>();
    nurbs
        .add_curve(
            "periodic NURBS",
            CurveDefinition::Nurbs {
                form: DocumentBSplineForm::Periodic,
                degree: 2,
                controls: nurbs_controls.to_vec(),
                gauge_weight: weights[0],
                weights,
                knots: vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
                span_ids: vec![0, 1, 2, 3, 4],
                next_span_id: 5,
            },
        )
        .unwrap();
    assert_one_complete_face(&nurbs, VisualProfileCurveFamily::PeriodicNurbs);
}

#[test]
fn analysis_is_read_only_and_reports_consumed_budgets() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let center = document.add_point("center", [0.0, 0.0]).unwrap();
    let radius = scalar(
        &mut document,
        "radius",
        1.0,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    );
    document
        .add_curve("circle", CurveDefinition::Circle { center, radius })
        .unwrap();
    let before = document.to_canonical_json().unwrap();
    let first = document.analyze_visual_profiles(VisualProfileOptions::default());
    let second = document.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(first, second);
    assert_eq!(document.to_canonical_json().unwrap(), before);
    assert_eq!(
        first.budgets.candidate_pairs.consumed,
        first.candidate_pairs
    );
    assert_eq!(first.budgets.fragments.consumed, first.fragment_count);
}

fn circle_document(centers: &[[f64; 2]], radius_value: f64) -> SketchDocument {
    let mut document = SketchDocument::new(radius_value.max(1.0)).unwrap();
    for (index, position) in centers.iter().copied().enumerate() {
        let center = document.add_point("center", position).unwrap();
        let radius = scalar(
            &mut document,
            &format!("radius {index}"),
            radius_value,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        );
        document
            .add_curve("circle", CurveDefinition::Circle { center, radius })
            .unwrap();
    }
    document
}

fn assert_public_counters_within_limits(analysis: &geosolve_sketch::VisualProfileAnalysis) {
    for counter in [
        analysis.budgets.candidate_pairs,
        analysis.budgets.intersection_subdivisions,
        analysis.budgets.intersection_roots,
        analysis.budgets.fragments,
        analysis.budgets.integration_subdivisions,
        analysis.budgets.containment_tests,
        analysis.budgets.faces,
    ] {
        assert!(
            counter.consumed <= counter.limit,
            "public counter exceeded its limit: {counter:?}; analysis={analysis:#?}"
        );
    }
}

#[test]
fn tangent_overlap_zero_speed_and_incomplete_components_are_typed() {
    let tangent = circle_document(&[[0.0, 0.0], [2.0, 0.0]], 1.0)
        .analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(tangent.status, VisualProfileStatus::Skipped);
    assert!(tangent.issues.iter().any(|issue| matches!(
        issue.kind,
        VisualProfileIssueKind::TangentIntersection { .. }
    )));

    let overlap = circle_document(&[[0.0, 0.0], [0.0, 0.0]], 1.0)
        .analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(overlap.status, VisualProfileStatus::Skipped);
    assert!(
        overlap
            .issues
            .iter()
            .any(|issue| matches!(issue.kind, VisualProfileIssueKind::CurveOverlap { .. }))
    );

    let mut cusp = SketchDocument::new(1.0).unwrap();
    let controls = [
        [-0.125, 0.25],
        [0.125, -1.0 / 12.0],
        [-0.125, -1.0 / 12.0],
        [0.125, 0.25],
    ]
    .map(|point| cusp.add_point("cusp", point).unwrap());
    cusp.add_curve("cusp", CurveDefinition::CubicBezier { controls })
        .unwrap();
    let cusp = cusp.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(cusp.status, VisualProfileStatus::Skipped);
    assert!(
        cusp.issues
            .iter()
            .any(|issue| matches!(issue.kind, VisualProfileIssueKind::ZeroSpeed { .. }))
    );

    let mut mixed = circle_document(&[[10.0, 0.0]], 1.0);
    let clean_curve = mixed.curves()[0].id;
    for position in [[0.0, 0.0], [2.0, 0.0]] {
        let center = mixed.add_point("tangent center", position).unwrap();
        let radius = scalar(
            &mut mixed,
            "tangent radius",
            1.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        );
        mixed
            .add_curve("tangent", CurveDefinition::Circle { center, radius })
            .unwrap();
    }
    let before = mixed.to_canonical_json().unwrap();
    let mixed_analysis = mixed.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(
        mixed_analysis.status,
        VisualProfileStatus::Truncated,
        "{mixed_analysis:#?}"
    );
    assert_eq!(mixed_analysis.faces.len(), 1, "{mixed_analysis:#?}");
    assert!(
        mixed_analysis.faces[0]
            .contours
            .iter()
            .flat_map(|contour| &contour.edges)
            .all(|edge| edge.source_span.curve == clean_curve),
        "{mixed_analysis:#?}"
    );
    assert!(mixed_analysis.issues.iter().any(|issue| {
        matches!(
            issue.kind,
            VisualProfileIssueKind::TangentIntersection { .. }
        ) && !issue
            .affected_spans
            .iter()
            .any(|span| span.curve == clean_curve)
    }));
    assert_eq!(mixed.to_canonical_json().unwrap(), before);
}

#[test]
fn nested_incomplete_component_taints_outer_disk_but_not_disjoint_face() {
    let mut document = SketchDocument::new(5.0).unwrap();
    let outer_center = document.add_point("outer center", [0.0, 0.0]).unwrap();
    let outer_radius = scalar(
        &mut document,
        "outer radius",
        5.0,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    );
    let outer = document
        .add_curve(
            "outer circle",
            CurveDefinition::Circle {
                center: outer_center,
                radius: outer_radius,
            },
        )
        .unwrap();
    for x in [-1.0, 1.0] {
        let center = document
            .add_point("inner tangent center", [x, 0.0])
            .unwrap();
        let radius = scalar(
            &mut document,
            "inner tangent radius",
            1.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        );
        document
            .add_curve(
                "inner tangent circle",
                CurveDefinition::Circle { center, radius },
            )
            .unwrap();
    }
    let far_center = document.add_point("far center", [20.0, 0.0]).unwrap();
    let far_radius = scalar(
        &mut document,
        "far radius",
        1.0,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    );
    let far = document
        .add_curve(
            "far clean circle",
            CurveDefinition::Circle {
                center: far_center,
                radius: far_radius,
            },
        )
        .unwrap();

    let analysis = document.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(
        analysis.status,
        VisualProfileStatus::Truncated,
        "{analysis:#?}"
    );
    assert_eq!(analysis.faces.len(), 1, "{analysis:#?}");
    assert!(
        analysis.faces[0]
            .contours
            .iter()
            .flat_map(|contour| &contour.edges)
            .all(|edge| edge.source_span.curve == far)
    );
    assert!(
        !analysis
            .faces
            .iter()
            .flat_map(|face| &face.contours)
            .flat_map(|contour| &contour.edges)
            .any(|edge| edge.source_span.curve == outer)
    );
    assert!(analysis.issues.iter().any(|issue| {
        matches!(
            issue.kind,
            VisualProfileIssueKind::ContainmentAmbiguity { .. }
        ) && issue.affected_spans.iter().any(|span| span.curve == outer)
    }));
}

#[test]
fn rational_pole_uncertainty_and_unresolved_pair_fail_closed() {
    let mut pole = SketchDocument::new(1.0).unwrap();
    let start = pole.add_point("pole start", [-1.0, 0.0]).unwrap();
    let end = pole.add_point("pole end", [1.0, 0.0]).unwrap();
    let weight = scalar(
        &mut pole,
        "near-pole weight",
        geosolve_sketch::MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
        ScalarUnit::Parameter,
        ScalarDomain::Bounded {
            lower: geosolve_sketch::MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
            upper: f64::MAX,
        },
    );
    let error = pole
        .add_curve(
            "near-pole rational conic",
            CurveDefinition::RationalQuadraticConic {
                start,
                weighted_middle: [0.0, 1.0],
                middle_weight: weight,
                end,
            },
        )
        .unwrap_err();
    assert!(error.to_string().contains("denominator"), "{error}");
    assert!(pole.curves().is_empty());
    let before = pole.to_canonical_json().unwrap();
    let _ = pole.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(pole.to_canonical_json().unwrap(), before);

    let mut unresolved = SketchDocument::new(2.0).unwrap();
    let vertex = unresolved.add_point("vertex", [0.0, 2.0]).unwrap();
    let focus = unresolved.add_point("focus", [0.0, 1.0]).unwrap();
    let trim_start = scalar(
        &mut unresolved,
        "trim start",
        -1.0,
        ScalarUnit::Parameter,
        ScalarDomain::Finite,
    );
    let trim_end = scalar(
        &mut unresolved,
        "trim end",
        1.0,
        ScalarUnit::Parameter,
        ScalarDomain::Finite,
    );
    unresolved
        .add_curve(
            "parabola",
            CurveDefinition::ParabolaSegment {
                vertex,
                focus,
                trim_start,
                trim_end,
            },
        )
        .unwrap();
    let controls = [[0.0, -2.0], [0.0, 0.0], [0.0, 2.0]]
        .map(|point| unresolved.add_point("quadratic", point).unwrap());
    unresolved
        .add_curve("quadratic", CurveDefinition::QuadraticBezier { controls })
        .unwrap();
    let analysis = unresolved.analyze_visual_profiles(VisualProfileOptions {
        max_intersection_depth: 0,
        ..VisualProfileOptions::default()
    });
    assert_eq!(
        analysis.status,
        VisualProfileStatus::Skipped,
        "{analysis:#?}"
    );
    assert!(
        analysis.issues.iter().any(|issue| matches!(
            issue.kind,
            VisualProfileIssueKind::UnresolvedIntersection { .. }
        )),
        "{analysis:#?}"
    );
    assert!(analysis.intersections.is_empty(), "{analysis:#?}");
}

#[test]
fn every_public_work_budget_fails_closed_with_consumed_evidence() {
    let two = circle_document(&[[-0.5, 0.0], [0.5, 0.0]], 1.0);
    let candidate = two.analyze_visual_profiles(VisualProfileOptions {
        max_candidate_pairs: 0,
        ..VisualProfileOptions::default()
    });
    assert_eq!(candidate.status, VisualProfileStatus::Skipped);
    assert_public_counters_within_limits(&candidate);
    assert_eq!(candidate.budgets.candidate_pairs.consumed, 0);
    assert!(matches!(
        candidate.issues[0].kind,
        VisualProfileIssueKind::CandidateBudgetExceeded { limit: 0, .. }
    ));

    let roots = two.analyze_visual_profiles(VisualProfileOptions {
        max_intersection_roots: 0,
        ..VisualProfileOptions::default()
    });
    assert_eq!(roots.status, VisualProfileStatus::Skipped);
    assert_public_counters_within_limits(&roots);
    assert!(matches!(
        roots.issues[0].kind,
        VisualProfileIssueKind::IntersectionRootBudgetExceeded { limit: 0, .. }
    ));

    let single = circle_document(&[[0.0, 0.0]], 1.0);
    let fragments = single.analyze_visual_profiles(VisualProfileOptions {
        max_fragments: 1,
        ..VisualProfileOptions::default()
    });
    assert_eq!(fragments.status, VisualProfileStatus::Skipped);
    assert_public_counters_within_limits(&fragments);
    assert_eq!(fragments.budgets.fragments.consumed, 0);
    assert!(matches!(
        fragments.issues[0].kind,
        VisualProfileIssueKind::FragmentBudgetExceeded { limit: 1, .. }
    ));

    let nested = circle_document(&[[0.0, 0.0], [0.0, 0.0]], 2.0);
    // Give the second circle a different radius without relying on solver state.
    let mut nested = nested;
    let CurveDefinition::Circle {
        radius: second_radius,
        ..
    } = nested.curves()[1].definition
    else {
        unreachable!();
    };
    nested.set_scalar_value(second_radius, 1.0).unwrap();
    let containment = nested.analyze_visual_profiles(VisualProfileOptions {
        max_containment_tests: 0,
        ..VisualProfileOptions::default()
    });
    assert_eq!(containment.status, VisualProfileStatus::Skipped);
    assert_public_counters_within_limits(&containment);
    assert!(matches!(
        containment.issues[0].kind,
        VisualProfileIssueKind::ContainmentBudgetExceeded { limit: 0, .. }
    ));

    let faces = single.analyze_visual_profiles(VisualProfileOptions {
        max_faces: 0,
        ..VisualProfileOptions::default()
    });
    assert_eq!(faces.status, VisualProfileStatus::Truncated);
    assert_public_counters_within_limits(&faces);
    assert_eq!(faces.budgets.faces.consumed, 0);
    assert!(matches!(
        faces.issues[0].kind,
        VisualProfileIssueKind::FaceBudgetExceeded { limit: 0, .. }
    ));

    let mut quadratic = SketchDocument::new(2.0).unwrap();
    let controls = [[-2.0, 1.0], [0.0, -2.0], [2.0, 1.0]]
        .map(|point| quadratic.add_point("control", point).unwrap());
    quadratic
        .add_curve("quadratic", CurveDefinition::QuadraticBezier { controls })
        .unwrap();
    let a = quadratic.add_point("a", [-3.0, 0.0]).unwrap();
    let b = quadratic.add_point("b", [3.0, 0.0]).unwrap();
    line(&mut quadratic, "line", a, b);
    let subdivision = quadratic.analyze_visual_profiles(VisualProfileOptions {
        max_intersection_subdivisions: 0,
        ..VisualProfileOptions::default()
    });
    assert_eq!(subdivision.status, VisualProfileStatus::Skipped);
    assert_public_counters_within_limits(&subdivision);
    assert!(subdivision.issues.iter().any(|issue| matches!(
        issue.kind,
        VisualProfileIssueKind::IntersectionSubdivisionBudgetExceeded { limit: 0, .. }
    )));

    let mut rational = SketchDocument::new(2.0).unwrap();
    let controls = [[-2.0, 0.0], [0.0, 3.0], [2.0, 0.0]]
        .map(|point| rational.add_point("control", point).unwrap());
    let weights = [1.0, 0.7, 1.0].map(|value| {
        scalar(
            &mut rational,
            "weight",
            value,
            ScalarUnit::Parameter,
            ScalarDomain::Positive,
        )
    });
    rational
        .add_curve(
            "rational",
            CurveDefinition::Nurbs {
                form: DocumentBSplineForm::Clamped,
                degree: 2,
                controls: controls.to_vec(),
                weights: weights.to_vec(),
                gauge_weight: weights[0],
                knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                span_ids: vec![0],
                next_span_id: 1,
            },
        )
        .unwrap();
    line(&mut rational, "close", controls[2], controls[0]);
    let integration = rational.analyze_visual_profiles(VisualProfileOptions {
        max_integration_subdivisions: 0,
        ..VisualProfileOptions::default()
    });
    assert_eq!(integration.status, VisualProfileStatus::Skipped);
    assert_public_counters_within_limits(&integration);
    assert!(integration.issues.iter().any(|issue| matches!(
        issue.kind,
        VisualProfileIssueKind::IntegrationBudgetExceeded { limit: 0, .. }
    )));
}

#[test]
fn shallow_rational_integration_depth_reports_area_uncertainty() {
    let mut document = SketchDocument::new(2.0).unwrap();
    let controls = [[-2.0, 0.0], [0.0, 3.0], [2.0, 0.0]]
        .map(|point| document.add_point("control", point).unwrap());
    let weights = [1.0, 0.7, 1.0].map(|value| {
        scalar(
            &mut document,
            "weight",
            value,
            ScalarUnit::Parameter,
            ScalarDomain::Positive,
        )
    });
    document
        .add_curve(
            "rational",
            CurveDefinition::Nurbs {
                form: DocumentBSplineForm::Clamped,
                degree: 2,
                controls: controls.to_vec(),
                weights: weights.to_vec(),
                gauge_weight: weights[0],
                knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                span_ids: vec![0],
                next_span_id: 1,
            },
        )
        .unwrap();
    line(&mut document, "close", controls[2], controls[0]);

    let analysis = document.analyze_visual_profiles(VisualProfileOptions {
        max_intersection_depth: 4,
        ..VisualProfileOptions::default()
    });
    assert_eq!(
        analysis.status,
        VisualProfileStatus::Skipped,
        "{analysis:#?}"
    );
    assert!(
        analysis
            .issues
            .iter()
            .any(|issue| matches!(issue.kind, VisualProfileIssueKind::AreaUncertainty { .. }))
    );
    assert!(analysis.faces.is_empty());
}

#[test]
fn certified_intersection_endpoints_increase_reported_area_uncertainty() {
    let standalone = circle_document(&[[0.0, 0.0]], 1.0)
        .analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(standalone.status, VisualProfileStatus::Complete);
    let baseline_uncertainty = standalone.faces[0].contours[0].area_uncertainty;

    let mut split = circle_document(&[[0.0, 0.0]], 1.0);
    let start = split.add_point("split start", [0.0, -2.0]).unwrap();
    let end = split.add_point("split end", [0.0, 2.0]).unwrap();
    line(&mut split, "splitter", start, end);
    let analysis = split.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(
        analysis.status,
        VisualProfileStatus::Complete,
        "{analysis:#?}"
    );
    assert_eq!(analysis.intersections.len(), 2, "{analysis:#?}");
    let endpoint_width = analysis
        .faces
        .iter()
        .flat_map(|face| &face.contours)
        .flat_map(|contour| &contour.edges)
        .flat_map(|edge| edge.source_parameter_enclosures)
        .map(|enclosure| enclosure[1] - enclosure[0])
        .fold(0.0, f64::max);
    let split_uncertainty = analysis
        .faces
        .iter()
        .flat_map(|face| &face.contours)
        .map(|contour| contour.area_uncertainty)
        .fold(0.0, f64::max);
    assert!(endpoint_width > 0.0, "{analysis:#?}");
    assert!(split_uncertainty > baseline_uncertainty, "{analysis:#?}");
    assert!(split_uncertainty > endpoint_width, "{analysis:#?}");
}

#[test]
fn close_shared_source_events_are_separated_or_fail_closed() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let base_start = document.add_point("base start", [0.0, 0.0]).unwrap();
    let base_end = document.add_point("base end", [1.0, 0.0]).unwrap();
    line(&mut document, "base", base_start, base_end);
    let first_x = 0.5_f64;
    let second_x = f64::from_bits(first_x.to_bits() + 4);
    let first_start = document.add_point("first start", [first_x, -1.0]).unwrap();
    let first_end = document.add_point("first end", [first_x, 0.0]).unwrap();
    line(&mut document, "first splitter", first_start, first_end);
    let second_start = document.add_point("second start", [second_x, 0.0]).unwrap();
    let second_end = document.add_point("second end", [second_x, 1.0]).unwrap();
    line(&mut document, "second splitter", second_start, second_end);

    let analysis = document.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(analysis.intersections.len(), 2, "{analysis:#?}");
    if analysis.status == VisualProfileStatus::Complete {
        assert_eq!(analysis.fragment_count, 7, "{analysis:#?}");
    } else {
        assert!(analysis.issues.iter().any(|issue| matches!(
            issue.kind,
            VisualProfileIssueKind::NumericalAmbiguity { .. }
                | VisualProfileIssueKind::UnresolvedIntersection { .. }
                | VisualProfileIssueKind::TangentIntersection { .. }
        )));
    }
}

#[derive(Clone, Copy, Debug)]
enum RepresentativeFaceKind {
    Circular,
    Polynomial,
    AnalyticConic,
    RationalSpline,
}

#[derive(Clone, Copy)]
struct FaceSimilarity {
    scale: f64,
    x_axis: [f64; 2],
    y_axis: [f64; 2],
    translation: [f64; 2],
}

impl FaceSimilarity {
    fn new(scale: f64, angle: f64, reflection: f64, translation: [f64; 2]) -> Self {
        let (sine, cosine) = angle.sin_cos();
        Self {
            scale,
            x_axis: [cosine, sine],
            y_axis: [-reflection * sine, reflection * cosine],
            translation,
        }
    }

    fn point(self, point: [f64; 2]) -> [f64; 2] {
        [
            self.translation[0]
                + self.scale * (self.x_axis[0] * point[0] + self.y_axis[0] * point[1]),
            self.translation[1]
                + self.scale * (self.x_axis[1] * point[0] + self.y_axis[1] * point[1]),
        ]
    }
}

fn representative_face_document(
    kind: RepresentativeFaceKind,
    similarity: FaceSimilarity,
) -> SketchDocument {
    let mut document = SketchDocument::new(4.0 * similarity.scale).unwrap();
    match kind {
        RepresentativeFaceKind::Circular => {
            let center = document
                .add_point("circle center", similarity.point([0.0, 0.0]))
                .unwrap();
            let radius = scalar(
                &mut document,
                "circle radius",
                2.0 * similarity.scale,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            );
            document
                .add_curve("circle", CurveDefinition::Circle { center, radius })
                .unwrap();
        }
        RepresentativeFaceKind::Polynomial => {
            let controls = [[-2.0, 0.0], [-1.0, 3.0], [1.0, 3.0], [2.0, 0.0]].map(|point| {
                document
                    .add_point("cubic control", similarity.point(point))
                    .unwrap()
            });
            document
                .add_curve("cubic", CurveDefinition::CubicBezier { controls })
                .unwrap();
            line(&mut document, "cubic closure", controls[3], controls[0]);
        }
        RepresentativeFaceKind::AnalyticConic => {
            let vertex = document
                .add_point("vertex", similarity.point([0.0, 0.0]))
                .unwrap();
            let focus = document
                .add_point("focus", similarity.point([0.0, 0.5]))
                .unwrap();
            let trim_start = scalar(
                &mut document,
                "parabola start",
                -2.0,
                ScalarUnit::Parameter,
                ScalarDomain::Finite,
            );
            let trim_end = scalar(
                &mut document,
                "parabola end",
                2.0,
                ScalarUnit::Parameter,
                ScalarDomain::Finite,
            );
            let curve = document
                .add_curve(
                    "parabola",
                    CurveDefinition::ParabolaSegment {
                        vertex,
                        focus,
                        trim_start,
                        trim_end,
                    },
                )
                .unwrap();
            close_derived_curve(&mut document, curve);
        }
        RepresentativeFaceKind::RationalSpline => {
            let controls = [[-2.0, 0.0], [0.0, 3.0], [2.0, 0.0]].map(|point| {
                document
                    .add_point("NURBS control", similarity.point(point))
                    .unwrap()
            });
            let weights = [1.0, 0.7, 1.0].map(|value| {
                scalar(
                    &mut document,
                    "NURBS weight",
                    value,
                    ScalarUnit::Parameter,
                    ScalarDomain::Positive,
                )
            });
            document
                .add_curve(
                    "NURBS",
                    CurveDefinition::Nurbs {
                        form: DocumentBSplineForm::Clamped,
                        degree: 2,
                        controls: controls.to_vec(),
                        weights: weights.to_vec(),
                        gauge_weight: weights[0],
                        knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                        span_ids: vec![0],
                        next_span_id: 1,
                    },
                )
                .unwrap();
            line(&mut document, "NURBS closure", controls[2], controls[0]);
        }
    }
    document
}

fn profile_topology_signature(
    analysis: &geosolve_sketch::VisualProfileAnalysis,
) -> (usize, usize, Vec<(usize, Vec<usize>)>) {
    (
        analysis.intersections.len(),
        analysis.faces.len(),
        analysis
            .faces
            .iter()
            .map(|face| {
                (
                    face.contours.len(),
                    face.contours
                        .iter()
                        .map(|contour| contour.edges.len())
                        .collect(),
                )
            })
            .collect(),
    )
}

#[test]
fn similarities_reflections_and_large_translations_preserve_curved_area() {
    for kind in [
        RepresentativeFaceKind::Circular,
        RepresentativeFaceKind::Polynomial,
        RepresentativeFaceKind::AnalyticConic,
        RepresentativeFaceKind::RationalSpline,
    ] {
        let baseline =
            representative_face_document(kind, FaceSimilarity::new(1.0, 0.0, 1.0, [0.0, 0.0]));
        let baseline_analysis = baseline.analyze_visual_profiles(VisualProfileOptions::default());
        assert_eq!(
            baseline_analysis.status,
            VisualProfileStatus::Complete,
            "{kind:?}"
        );
        assert_eq!(
            baseline_analysis.faces.len(),
            1,
            "{kind:?}: {baseline_analysis:#?}"
        );
        let baseline_topology = profile_topology_signature(&baseline_analysis);
        let baseline_face = &baseline_analysis.faces[0];
        for scale in [1.0e-6_f64, 1.0, 1.0e6] {
            for reflection in [-1.0, 1.0] {
                let translation_scale = scale * 100.0;
                let similarity = FaceSimilarity::new(
                    scale,
                    0.37 + 0.11 * reflection,
                    reflection,
                    [translation_scale, -2.0 * translation_scale],
                );
                let document = representative_face_document(kind, similarity);
                let before = document.to_canonical_json().unwrap();
                let analysis = document.analyze_visual_profiles(VisualProfileOptions::default());
                assert_eq!(
                    analysis.status,
                    VisualProfileStatus::Complete,
                    "{kind:?}, scale={scale}, reflection={reflection}: {analysis:#?}"
                );
                assert_eq!(analysis.families, baseline_analysis.families, "{kind:?}");
                assert_eq!(
                    profile_topology_signature(&analysis),
                    baseline_topology,
                    "{kind:?}"
                );
                assert_eq!(analysis.faces.len(), 1, "{kind:?}");
                let face = &analysis.faces[0];
                let expected_area = baseline_face.visual_area * scale * scale;
                let published_tolerance = face.area_uncertainty
                    + baseline_face.area_uncertainty * scale * scale
                    + document.model_scale() * document.model_scale() * 1.0e-9;
                assert!(
                    (face.visual_area - expected_area).abs() <= published_tolerance,
                    "{kind:?}, scale={scale}, reflection={reflection}: area={}, expected={expected_area}, tolerance={published_tolerance}, analysis={analysis:#?}",
                    face.visual_area
                );
                assert_eq!(document.to_canonical_json().unwrap(), before, "{kind:?}");
            }
        }
    }
}

#[test]
fn fillet_owned_trim_joins_are_fresh_and_persistence_neutral() {
    let fixture = alpha_scenario(AlphaScenarioKind::M28TrimmedFillet, 1.0).unwrap();
    let before = fixture.document.to_canonical_json().unwrap();
    let analysis = fixture
        .document
        .analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(
        analysis.status,
        VisualProfileStatus::Complete,
        "{analysis:#?}"
    );
    assert!(analysis.issues.is_empty(), "{analysis:#?}");
    assert!(
        analysis
            .families
            .contains(&VisualProfileCurveFamily::CircularArc)
    );
    assert_eq!(fixture.document.to_canonical_json().unwrap(), before);
    assert!(!fixture.document.trim_views().is_empty());
    assert!(fixture.document.trim_views().iter().any(|view| {
        let interval = fixture.document.visible_interval(view.support).unwrap();
        interval.start.to_bits() != 0.0_f64.to_bits() || interval.end.to_bits() != 1.0_f64.to_bits()
    }));
}

#[allow(clippy::cast_possible_truncation)]
fn periodic_parameter(total: f64) -> (f64, i32) {
    let principal = total.rem_euclid(std::f64::consts::TAU);
    let winding = ((total - principal) / std::f64::consts::TAU).round() as i32;
    (principal, winding)
}

#[test]
fn supporting_line_tangency_outside_visible_interval_is_ignored() {
    let mut document = SketchDocument::new(2.0).unwrap();
    let start = document.add_point("line start", [0.0, 0.0]).unwrap();
    let end = document.add_point("line end", [1.0, 0.0]).unwrap();
    line(&mut document, "finite line", start, end);
    let center = document.add_point("circle center", [2.0, 1.0]).unwrap();
    let radius = scalar(
        &mut document,
        "circle radius",
        1.0,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    );
    document
        .add_curve("circle", CurveDefinition::Circle { center, radius })
        .unwrap();

    let analysis = document.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(
        analysis.status,
        VisualProfileStatus::Complete,
        "{analysis:#?}"
    );
    assert!(analysis.issues.is_empty(), "{analysis:#?}");
    assert!(analysis.intersections.is_empty(), "{analysis:#?}");
    assert_eq!(analysis.faces.len(), 1, "{analysis:#?}");
}

fn same_carrier_arc(
    document: &mut SketchDocument,
    label: &str,
    center: geosolve_sketch::DesignPointId,
    radius: geosolve_sketch::DesignScalarId,
    start: f64,
    end: f64,
) -> geosolve_sketch::CurveId {
    let start_angle = scalar(
        document,
        &format!("{label} start"),
        start,
        ScalarUnit::Angle,
        ScalarDomain::Finite,
    );
    let end_angle = scalar(
        document,
        &format!("{label} end"),
        end,
        ScalarUnit::Angle,
        ScalarDomain::Finite,
    );
    document
        .add_curve(
            label,
            CurveDefinition::CircularArc {
                center,
                radius,
                start_angle,
                end_angle,
                sweep: DocumentArcSweep::CounterClockwise,
            },
        )
        .unwrap()
}

#[test]
fn same_carrier_circular_overlap_uses_angular_intervals() {
    let mut disjoint = SketchDocument::new(2.0).unwrap();
    let center = disjoint.add_point("center", [0.0, 0.0]).unwrap();
    let radius = scalar(
        &mut disjoint,
        "radius",
        2.0,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    );
    same_carrier_arc(&mut disjoint, "first", center, radius, 0.0, 2.0);
    let second_radius = scalar(
        &mut disjoint,
        "second radius",
        2.0,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    );
    same_carrier_arc(&mut disjoint, "second", center, second_radius, 2.5, 5.5);
    let analysis = disjoint.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(
        analysis.status,
        VisualProfileStatus::Complete,
        "{analysis:#?}"
    );
    assert!(analysis.issues.is_empty(), "{analysis:#?}");

    let mut overlap = SketchDocument::new(2.0).unwrap();
    let center = overlap.add_point("center", [0.0, 0.0]).unwrap();
    let radius = scalar(
        &mut overlap,
        "radius",
        2.0,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    );
    same_carrier_arc(&mut overlap, "first", center, radius, 5.5, 6.2);
    let second_radius = scalar(
        &mut overlap,
        "second radius",
        2.0,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    );
    same_carrier_arc(&mut overlap, "second", center, second_radius, -0.5, 0.2);
    let analysis = overlap.analyze_visual_profiles(VisualProfileOptions::default());
    assert_ne!(
        analysis.status,
        VisualProfileStatus::Complete,
        "{analysis:#?}"
    );
    assert!(
        analysis
            .issues
            .iter()
            .any(|issue| matches!(issue.kind, VisualProfileIssueKind::CurveOverlap { .. })),
        "{analysis:#?}"
    );
}

fn transformed_fillet_profile(reflection: f64) -> (SketchDocument, [geosolve_sketch::CurveId; 3]) {
    let rotation = 0.63;
    let similarity = FaceSimilarity::new(1.0, rotation, reflection, [7.0, -4.0]);
    let mut document = SketchDocument::new(4.0).unwrap();
    let circle_center = document
        .add_point("transformed circle center", similarity.point([0.0, 0.0]))
        .unwrap();
    let circle_radius = scalar(
        &mut document,
        "transformed circle radius",
        2.0,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    );
    let circle = document
        .add_curve(
            "transformed circle parent",
            CurveDefinition::Circle {
                center: circle_center,
                radius: circle_radius,
            },
        )
        .unwrap();
    let line_start = document
        .add_point("transformed line start", similarity.point([0.0, 1.0]))
        .unwrap();
    let line_end = document
        .add_point("transformed line end", similarity.point([6.0, 1.0]))
        .unwrap();
    let parent_line = line(
        &mut document,
        "transformed line parent",
        line_start,
        line_end,
    );
    let (contact_parameter, contact_winding) = periodic_parameter(rotation);
    let (anchor_parameter, anchor_winding) = periodic_parameter(rotation - std::f64::consts::PI);
    let fillet = document
        .add_curve_curve_fillet(
            "transformed owned fillet",
            CurveCurveFilletRequest {
                first: CurveFilletParentRequest {
                    curve: CurveSpan::line(circle),
                    parameter: contact_parameter,
                    winding: contact_winding,
                    neighborhood: ContactNeighborhood::Local {
                        lower: rotation - 0.4,
                        upper: rotation + 0.4,
                    },
                    side: DocumentCurveNormalSide::Right,
                    trim_endpoint: DocumentFilletTrimEndpoint::End,
                    periodic_anchor: Some(DocumentTrimParameter {
                        parameter: anchor_parameter,
                        winding: anchor_winding,
                    }),
                },
                second: CurveFilletParentRequest {
                    curve: CurveSpan::line(parent_line),
                    parameter: 0.5,
                    winding: 0,
                    neighborhood: ContactNeighborhood::Interior,
                    side: if reflection > 0.0 {
                        DocumentCurveNormalSide::Right
                    } else {
                        DocumentCurveNormalSide::Left
                    },
                    trim_endpoint: DocumentFilletTrimEndpoint::Start,
                    periodic_anchor: None,
                },
                endpoint_order: DocumentFilletEndpointOrder::SecondThenFirst,
                sweep: if reflection > 0.0 {
                    DocumentArcSweep::CounterClockwise
                } else {
                    DocumentArcSweep::Clockwise
                },
                radius: 1.0,
                radius_mode: DocumentDimensionMode::Driving,
            },
        )
        .unwrap();
    let closure_point = document
        .add_point(
            "transformed circle closure",
            similarity.point([
                std::f64::consts::SQRT_2,
                -reflection * std::f64::consts::SQRT_2,
            ]),
        )
        .unwrap();
    line(
        &mut document,
        "transformed ordinary closure",
        line_end,
        closure_point,
    );
    let closure_total = rotation - 0.25 * std::f64::consts::PI;
    let (closure_parameter, closure_winding) = periodic_parameter(closure_total);
    let closure_contact = document
        .add_curve_contact(
            "transformed closure contact",
            CurveSpan::line(circle),
            closure_parameter,
            closure_winding,
            ContactNeighborhood::Local {
                lower: closure_total - 0.1,
                upper: closure_total + 0.1,
            },
            None,
        )
        .unwrap();
    document
        .add_constraint(
            "transformed closure join",
            DocumentConstraintDefinition::PointOnCurve {
                point: closure_point,
                contact: closure_contact,
            },
        )
        .unwrap();
    (document, [parent_line, circle, fillet.arc])
}

#[test]
fn rotated_and_reflected_fillet_endpoints_weld_only_through_owned_topology() {
    for reflection in [-1.0, 1.0] {
        let (document, required) = transformed_fillet_profile(reflection);
        let analysis = document.analyze_visual_profiles(VisualProfileOptions::default());
        assert_eq!(
            analysis.status,
            VisualProfileStatus::Complete,
            "reflection={reflection}: {analysis:#?}"
        );
        assert!(
            analysis.issues.is_empty(),
            "reflection={reflection}: {analysis:#?}"
        );
        assert!(
            analysis.faces.iter().any(|face| {
                face.contours.iter().any(|contour| {
                    let curves = contour
                        .edges
                        .iter()
                        .map(|edge| edge.source_span.curve)
                        .collect::<std::collections::BTreeSet<_>>();
                    required.iter().all(|curve| curves.contains(curve))
                })
            }),
            "reflection={reflection}: {analysis:#?}"
        );
    }
}

const PROFILE_SCENARIOS: [AlphaScenarioKind; 6] = [
    AlphaScenarioKind::ProfileAllFamilies,
    AlphaScenarioKind::ProfileCurvedTopology,
    AlphaScenarioKind::ProfileFilletTrim,
    AlphaScenarioKind::ProfileNurbsSelfIntersection,
    AlphaScenarioKind::ProfileIncomplete,
    AlphaScenarioKind::ProfileBudget,
];

#[test]
fn reusable_profile_scenarios_publish_exact_metadata_and_evidence() {
    let expected = [
        ("profile-all-families", 0xfe_0000_u128),
        ("profile-curved-topology", 0xff_0000),
        ("profile-fillet-trim", 0x100_0000),
        ("profile-nurbs-self-intersection", 0x103_0000),
        ("profile-incomplete", 0x101_0000),
        ("profile-budget", 0x102_0000),
    ];
    for (kind, (key, namespace)) in PROFILE_SCENARIOS.into_iter().zip(expected) {
        assert_eq!(kind.key(), key);
        assert!(kind.uat().is_none());
        let uat = kind.profile_uat().expect("profile UAT metadata");
        assert!(!uat.title.is_empty());
        assert!(!uat.instructions.is_empty());

        let fixture = alpha_scenario(kind, 1.0).unwrap();
        assert_eq!(fixture.document.id().0.as_u128(), namespace);
        let before = fixture.document.to_canonical_json().unwrap();
        let analysis = fixture.document.analyze_visual_profiles(uat.options);
        assert_eq!(analysis.status, uat.expected_status, "{}", kind.key());
        assert_eq!(
            analysis.families.len(),
            uat.expected_family_count,
            "{}: {analysis:#?}",
            kind.key()
        );
        assert!(
            analysis.faces.len() >= uat.expected_minimum_face_count,
            "{}: {analysis:#?}",
            kind.key()
        );
        assert_eq!(fixture.document.to_canonical_json().unwrap(), before);
        assert_eq!(
            SketchDocument::from_json(&before)
                .unwrap()
                .to_canonical_json()
                .unwrap(),
            before
        );

        match (kind, fixture.ids) {
            (AlphaScenarioKind::ProfileAllFamilies, AlphaScenarioIds::ProfileAllFamilies(ids)) => {
                assert_eq!(ids.curves.len(), 15);
                assert!(analysis.intersections.len() >= 20, "{analysis:#?}");
                for curve in ids.curves {
                    assert!(
                        analysis
                            .faces
                            .iter()
                            .flat_map(|face| &face.contours)
                            .flat_map(|contour| &contour.edges)
                            .any(|edge| edge.source_span.curve == curve),
                        "curve {curve:?} did not participate in a face: {analysis:#?}"
                    );
                }
            }
            (
                AlphaScenarioKind::ProfileCurvedTopology,
                AlphaScenarioIds::ProfileCurvedTopology(ids),
            ) => {
                assert_eq!(ids.curves.len(), 4);
                assert!(analysis.intersections.len() >= 4, "{analysis:#?}");
                assert!(analysis.intersections.iter().any(|intersection| {
                    (intersection.first_span.curve == ids.curves[0]
                        && intersection.second_span.curve == ids.curves[1])
                        || (intersection.first_span.curve == ids.curves[1]
                            && intersection.second_span.curve == ids.curves[0])
                }));
                assert!(
                    analysis.faces.iter().any(|face| face.contours.len() >= 2),
                    "{analysis:#?}"
                );
            }
            (AlphaScenarioKind::ProfileFilletTrim, AlphaScenarioIds::ProfileFilletTrim(ids)) => {
                assert_eq!(ids.fillet.contacts.len(), 2);
                assert_eq!(fixture.document.trim_views().len(), 2);
                assert!(analysis.issues.is_empty(), "{analysis:#?}");
                assert!(
                    analysis.faces.iter().any(|face| {
                        face.contours.iter().any(|contour| {
                            let curves = contour
                                .edges
                                .iter()
                                .map(|edge| edge.source_span.curve)
                                .collect::<std::collections::BTreeSet<_>>();
                            curves.contains(&ids.line)
                                && curves.contains(&ids.circle)
                                && curves.contains(&ids.fillet.arc)
                        })
                    }),
                    "fillet ownership did not weld one line/circle/output-arc traversal: {analysis:#?}"
                );
            }
            (
                AlphaScenarioKind::ProfileNurbsSelfIntersection,
                AlphaScenarioIds::ProfileNurbsSelfIntersection(ids),
            ) => {
                assert_eq!(ids.controls.len(), 4);
                assert_eq!(ids.weights.len(), 4);
                let roots = analysis
                    .intersections
                    .iter()
                    .filter(|root| {
                        root.first_span.curve == ids.curve && root.second_span.curve == ids.curve
                    })
                    .count();
                assert_eq!(roots, 1, "{analysis:#?}");
                assert!(analysis.issues.is_empty(), "{analysis:#?}");
            }
            (AlphaScenarioKind::ProfileIncomplete, AlphaScenarioIds::ProfileIncomplete(ids)) => {
                assert_eq!(ids.curves.len(), 3);
                assert!(
                    analysis
                        .faces
                        .iter()
                        .flat_map(|face| &face.contours)
                        .flat_map(|contour| &contour.edges)
                        .any(|edge| edge.source_span.curve == ids.curves[0])
                );
                assert!(analysis.issues.iter().any(|issue| matches!(
                    issue.kind,
                    VisualProfileIssueKind::TangentIntersection { .. }
                )));
            }
            (AlphaScenarioKind::ProfileBudget, AlphaScenarioIds::ProfileBudget(ids)) => {
                assert_eq!(ids.curves.len(), 4);
                assert!(analysis.faces.is_empty(), "{analysis:#?}");
                assert!(analysis.issues.iter().any(|issue| matches!(
                    issue.kind,
                    VisualProfileIssueKind::IntersectionRootBudgetExceeded { limit: 0, .. }
                )));
                let default_analysis = fixture
                    .document
                    .analyze_visual_profiles(VisualProfileOptions::default());
                assert_eq!(
                    default_analysis.status,
                    VisualProfileStatus::Complete,
                    "{default_analysis:#?}"
                );
                assert!(!default_analysis.faces.is_empty(), "{default_analysis:#?}");
            }
            _ => panic!("profile scenario returned mismatched IDs"),
        }
    }
}

#[test]
fn explicit_interior_contact_closes_fillet_profile_within_accepted_residual() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let mut fixture = alpha_scenario(AlphaScenarioKind::ProfileFilletTrim, scale).unwrap();
        let closure = fixture
            .document
            .points()
            .iter()
            .find(|point| point.label == "Profile fillet circle closure")
            .expect("profile closure point")
            .id;
        let contact = fixture
            .document
            .contacts()
            .iter()
            .find(|contact| contact.label == "Profile fillet circle closure contact")
            .expect("profile closure contact")
            .id;
        let contact_position = fixture
            .document
            .evaluate_contact_jet(contact)
            .unwrap()
            .position;
        let radius = contact_position.x.hypot(contact_position.y);
        let accepted_offset = 0.5e-9 * fixture.document.model_scale();
        fixture
            .document
            .set_point_position(
                closure,
                [
                    contact_position.x + accepted_offset * contact_position.x / radius,
                    contact_position.y + accepted_offset * contact_position.y / radius,
                ],
            )
            .unwrap();

        let analysis = fixture
            .document
            .analyze_visual_profiles(VisualProfileOptions::default());
        assert_eq!(
            analysis.status,
            VisualProfileStatus::Complete,
            "scale={scale}: {analysis:#?}"
        );
        assert!(analysis.issues.is_empty(), "scale={scale}: {analysis:#?}");
        assert!(!analysis.faces.is_empty(), "scale={scale}: {analysis:#?}");
    }
}

#[test]
fn all_family_profile_scenario_is_scale_invariant() {
    let mut expected_ids = None;
    for scale in [1.0e-6, 1.0e6] {
        let fixture = alpha_scenario(AlphaScenarioKind::ProfileAllFamilies, scale).unwrap();
        assert_eq!(
            fixture.document.model_scale().to_bits(),
            (10.0 * scale).to_bits()
        );
        let AlphaScenarioIds::ProfileAllFamilies(ids) = fixture.ids else {
            panic!("all-family profile IDs")
        };
        if let Some(expected) = &expected_ids {
            assert_eq!(&ids, expected);
        } else {
            expected_ids = Some(ids.clone());
        }
        let uat = AlphaScenarioKind::ProfileAllFamilies.profile_uat().unwrap();
        let before = fixture.document.to_canonical_json().unwrap();
        let analysis = fixture.document.analyze_visual_profiles(uat.options);
        assert_eq!(
            analysis.status,
            VisualProfileStatus::Complete,
            "{analysis:#?}"
        );
        assert_eq!(analysis.families.len(), 15, "{analysis:#?}");
        assert!(analysis.faces.len() >= 15, "{analysis:#?}");
        assert_eq!(fixture.document.to_canonical_json().unwrap(), before);
    }
}
