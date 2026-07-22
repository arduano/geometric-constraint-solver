// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(clippy::too_many_lines)]

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use geosolve_core::{AuditEvaluationStatus, HardValidity, SolverConfig};
use geosolve_geometry::{
    BSplineCurve2, BSplineForm, CurveJet2, CurveParameterDomain, DirectedParameterTrim,
    HyperbolaBranch, NurbsCurve2, Point2, Vector2, circle_jet, circular_arc_jet, line_jet,
};
use geosolve_sketch::{
    ArcSweep, ConicScalarRole, ConicVectorRole, ContactNeighborhood, CurveContactNeighborhood,
    CurveCurveFilletRequest, CurveDefinition, CurveFilletParentRequest, CurveNormalSide, CurveSpan,
    DimensionMode, DocumentArcSweep, DocumentBSplineForm, DocumentCommand, DocumentCommandEffect,
    DocumentConstraintDefinition, DocumentCurveNormalSide, DocumentDimensionMode, DocumentEdit,
    DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint, DocumentSolveRequest,
    FilletEndpointOrder, LineParameterDomain, RuntimeCurve, RuntimeSource, ScalarDomain,
    ScalarUnit, Sketch, SketchCurve, SketchCurveContact, SketchDocument, SketchDocumentSession,
    SketchSolveRequest, SketchSource,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Family {
    Line,
    Circle,
    CircularArc,
    Ellipse,
    EllipticalArc,
    RationalQuadraticConic,
    ParabolaSegment,
    HyperbolaSegment,
    QuadraticBezier,
    CubicBezier,
    ClampedBSpline,
    PeriodicBSpline,
    ClampedNurbs,
    PeriodicNurbs,
}

const FAMILIES: [Family; 14] = [
    Family::Line,
    Family::Circle,
    Family::CircularArc,
    Family::Ellipse,
    Family::EllipticalArc,
    Family::RationalQuadraticConic,
    Family::ParabolaSegment,
    Family::HyperbolaSegment,
    Family::QuadraticBezier,
    Family::CubicBezier,
    Family::ClampedBSpline,
    Family::PeriodicBSpline,
    Family::ClampedNurbs,
    Family::PeriodicNurbs,
];

impl Family {
    const fn label(self) -> &'static str {
        match self {
            Self::Line => "line",
            Self::Circle => "circle",
            Self::CircularArc => "circular arc",
            Self::Ellipse => "ellipse",
            Self::EllipticalArc => "elliptical arc",
            Self::RationalQuadraticConic => "rational quadratic conic",
            Self::ParabolaSegment => "parabola segment",
            Self::HyperbolaSegment => "hyperbola segment",
            Self::QuadraticBezier => "quadratic Bezier",
            Self::CubicBezier => "cubic Bezier",
            Self::ClampedBSpline => "clamped B-spline",
            Self::PeriodicBSpline => "periodic B-spline",
            Self::ClampedNurbs => "clamped NURBS",
            Self::PeriodicNurbs => "periodic NURBS",
        }
    }
}

#[derive(Clone, Debug)]
struct NurbsIncidence {
    id: geosolve_sketch::NurbsId,
    active_support: Vec<usize>,
    gauge_index: usize,
    control_count: usize,
}

#[derive(Clone, Debug)]
struct ParentFixture {
    family: Family,
    contact: SketchCurveContact,
    parameter: f64,
    winding: i32,
    active_points: Vec<geosolve_sketch::PointId>,
    inactive_points: Vec<geosolve_sketch::PointId>,
    circle: Option<geosolve_sketch::CircleId>,
    arc: Option<geosolve_sketch::ArcId>,
    conic_scalar: Option<(geosolve_sketch::ConicId, ConicScalarRole)>,
    conic_vector: Option<(geosolve_sketch::ConicId, ConicVectorRole)>,
    nurbs: Option<NurbsIncidence>,
}

#[derive(Clone, Copy)]
struct Similarity {
    scale: f64,
    cosine: f64,
    sine: f64,
    base: Point2<f64>,
    target: Point2<f64>,
}

impl Similarity {
    fn new(
        scale: f64,
        base: Point2<f64>,
        base_tangent: Vector2<f64>,
        target: Point2<f64>,
        target_tangent: Vector2<f64>,
    ) -> Self {
        let base_tangent = base_tangent.normalize();
        let target_tangent = target_tangent.normalize();
        Self {
            scale,
            cosine: base_tangent.dot(&target_tangent),
            sine: base_tangent.x * target_tangent.y - base_tangent.y * target_tangent.x,
            base,
            target,
        }
    }

    fn vector(self, vector: Vector2<f64>) -> Vector2<f64> {
        Vector2::new(
            self.scale * (self.cosine * vector.x - self.sine * vector.y),
            self.scale * (self.sine * vector.x + self.cosine * vector.y),
        )
    }

    fn point(self, point: Point2<f64>) -> Point2<f64> {
        self.target + self.vector(point - self.base)
    }
}

fn unit(angle: f64) -> Vector2<f64> {
    Vector2::new(angle.cos(), angle.sin())
}

fn left_normal(tangent: Vector2<f64>) -> Vector2<f64> {
    Vector2::new(-tangent.y, tangent.x)
}

fn map_local(
    target: Point2<f64>,
    tangent: Vector2<f64>,
    scale: f64,
    local: [f64; 2],
) -> Point2<f64> {
    target
        + tangent.normalize() * (scale * local[0])
        + left_normal(tangent.normalize()) * (scale * local[1])
}

fn add_points(
    sketch: &mut Sketch,
    label: &str,
    positions: &[Point2<f64>],
) -> Vec<geosolve_sketch::PointId> {
    positions
        .iter()
        .enumerate()
        .map(|(index, position)| {
            sketch
                .add_named_point(format!("{label} control {index}"), *position)
                .unwrap()
        })
        .collect()
}

fn explicit_local(parameter: f64, half_width: f64) -> CurveContactNeighborhood {
    CurveContactNeighborhood::Local {
        lower: parameter - half_width,
        upper: parameter + half_width,
    }
}

#[allow(clippy::too_many_lines)]
fn add_parent(
    sketch: &mut Sketch,
    family: Family,
    target: Point2<f64>,
    tangent: Vector2<f64>,
    scale: f64,
    winding: i32,
) -> ParentFixture {
    let tangent = tangent.normalize();
    let normal = left_normal(tangent);
    let label = family.label();
    let fixture = match family {
        Family::Line => {
            let points = add_points(
                sketch,
                label,
                &[
                    target - tangent * (2.0 * scale),
                    target + tangent * (2.0 * scale),
                ],
            );
            let segment = sketch
                .add_named_segment(label, points[0], points[1])
                .unwrap();
            ParentFixture {
                family,
                contact: SketchCurveContact {
                    curve: SketchCurve::Line {
                        segment,
                        domain: LineParameterDomain::BoundedSegment,
                    },
                    parameter: 0.5,
                    neighborhood: explicit_local(0.5, 0.3),
                },
                parameter: 0.5,
                winding,
                active_points: points,
                inactive_points: Vec::new(),
                circle: None,
                arc: None,
                conic_scalar: None,
                conic_vector: None,
                nurbs: None,
            }
        }
        Family::Circle => {
            let parent_radius = 2.0 * scale;
            let center = sketch
                .add_named_point(format!("{label} center"), target + normal * parent_radius)
                .unwrap();
            let circle = sketch
                .add_named_circle(label, center, parent_radius)
                .unwrap();
            let parameter = tangent.y.atan2(tangent.x) - FRAC_PI_2 + f64::from(winding) * TAU;
            ParentFixture {
                family,
                contact: SketchCurveContact {
                    curve: SketchCurve::Circle(circle),
                    parameter,
                    neighborhood: explicit_local(parameter, 0.35),
                },
                parameter,
                winding,
                active_points: vec![center],
                inactive_points: Vec::new(),
                circle: Some(circle),
                arc: None,
                conic_scalar: None,
                conic_vector: None,
                nurbs: None,
            }
        }
        Family::CircularArc => {
            let parent_radius = 2.0 * scale;
            let center = sketch
                .add_named_point(format!("{label} center"), target + normal * parent_radius)
                .unwrap();
            let contact_angle = tangent.y.atan2(tangent.x) - FRAC_PI_2;
            let arc = sketch
                .add_named_arc(
                    label,
                    center,
                    parent_radius,
                    contact_angle - FRAC_PI_2,
                    contact_angle + FRAC_PI_2,
                    ArcSweep::CounterClockwise,
                )
                .unwrap();
            ParentFixture {
                family,
                contact: SketchCurveContact {
                    curve: SketchCurve::Arc(arc),
                    parameter: 0.5,
                    neighborhood: explicit_local(0.5, 0.3),
                },
                parameter: 0.5,
                winding,
                active_points: vec![center],
                inactive_points: Vec::new(),
                circle: None,
                arc: Some(arc),
                conic_scalar: None,
                conic_vector: None,
                nurbs: None,
            }
        }
        Family::Ellipse | Family::EllipticalArc => {
            let center = sketch
                .add_named_point(format!("{label} center"), target + normal * scale)
                .unwrap();
            let axis = sketch
                .add_named_point(
                    format!("{label} axis"),
                    target + normal * scale + tangent * (2.0 * scale),
                )
                .unwrap();
            let (conic, parameter) = if family == Family::Ellipse {
                (
                    sketch.add_named_ellipse(label, center, axis, 0.5).unwrap(),
                    -FRAC_PI_2 + f64::from(winding) * TAU,
                )
            } else {
                (
                    sketch
                        .add_named_elliptical_arc(label, center, axis, 0.5, -PI, PI)
                        .unwrap(),
                    0.5,
                )
            };
            ParentFixture {
                family,
                contact: SketchCurveContact {
                    curve: SketchCurve::Conic(conic),
                    parameter,
                    neighborhood: explicit_local(
                        parameter,
                        if family == Family::Ellipse { 0.35 } else { 0.3 },
                    ),
                },
                parameter,
                winding,
                active_points: vec![center, axis],
                inactive_points: Vec::new(),
                circle: None,
                arc: None,
                conic_scalar: Some((conic, ConicScalarRole::MinorAxisRatio)),
                conic_vector: None,
                nurbs: None,
            }
        }
        Family::RationalQuadraticConic => {
            let weight = 0.75;
            let start_position = map_local(target, tangent, scale, [-1.0, -weight]);
            let end_position = map_local(target, tangent, scale, [1.0, -weight]);
            let points = add_points(sketch, label, &[start_position, end_position]);
            let weighted_middle = target.coords * weight + normal * (weight * scale);
            let conic = sketch
                .add_named_rational_quadratic(label, points[0], weighted_middle, weight, points[1])
                .unwrap();
            ParentFixture {
                family,
                contact: SketchCurveContact {
                    curve: SketchCurve::Conic(conic),
                    parameter: 0.5,
                    neighborhood: explicit_local(0.5, 0.3),
                },
                parameter: 0.5,
                winding,
                active_points: points,
                inactive_points: Vec::new(),
                circle: None,
                arc: None,
                conic_scalar: Some((conic, ConicScalarRole::MiddleWeight)),
                conic_vector: Some((conic, ConicVectorRole::WeightedMiddle)),
                nurbs: None,
            }
        }
        Family::ParabolaSegment => {
            let points = add_points(sketch, label, &[target, target - normal * (0.5 * scale)]);
            let conic = sketch
                .add_named_parabola_segment(
                    label,
                    points[0],
                    points[1],
                    DirectedParameterTrim::try_new(-1.0, 1.0).unwrap(),
                )
                .unwrap();
            ParentFixture {
                family,
                contact: SketchCurveContact {
                    curve: SketchCurve::Conic(conic),
                    parameter: 0.5,
                    neighborhood: explicit_local(0.5, 0.3),
                },
                parameter: 0.5,
                winding,
                active_points: points,
                inactive_points: Vec::new(),
                circle: None,
                arc: None,
                conic_scalar: None,
                conic_vector: None,
                nurbs: None,
            }
        }
        Family::HyperbolaSegment => {
            let center_position = target + normal * scale;
            let points = add_points(sketch, label, &[center_position, target]);
            let conic = sketch
                .add_named_hyperbola_segment(
                    label,
                    points[0],
                    points[1],
                    0.75 * scale,
                    HyperbolaBranch::Positive,
                    DirectedParameterTrim::try_new(-1.0, 1.0).unwrap(),
                )
                .unwrap();
            ParentFixture {
                family,
                contact: SketchCurveContact {
                    curve: SketchCurve::Conic(conic),
                    parameter: 0.5,
                    neighborhood: explicit_local(0.5, 0.3),
                },
                parameter: 0.5,
                winding,
                active_points: points,
                inactive_points: Vec::new(),
                circle: None,
                arc: None,
                conic_scalar: Some((conic, ConicScalarRole::SemiConjugate)),
                conic_vector: None,
                nurbs: None,
            }
        }
        Family::QuadraticBezier | Family::CubicBezier => {
            let local = if family == Family::QuadraticBezier {
                vec![[-1.0, -0.25], [0.0, 0.25], [1.0, -0.25]]
            } else {
                vec![[-1.5, -1.5], [-0.5, 0.5], [0.5, 0.5], [1.5, -1.5]]
            };
            let points = add_points(
                sketch,
                label,
                &local
                    .into_iter()
                    .map(|point| map_local(target, tangent, scale, point))
                    .collect::<Vec<_>>(),
            );
            let bezier = if family == Family::QuadraticBezier {
                sketch
                    .add_quadratic_bezier(label, [points[0], points[1], points[2]])
                    .unwrap()
            } else {
                sketch
                    .add_cubic_bezier(label, [points[0], points[1], points[2], points[3]])
                    .unwrap()
            };
            ParentFixture {
                family,
                contact: SketchCurveContact {
                    curve: SketchCurve::Bezier(bezier),
                    parameter: 0.5,
                    neighborhood: explicit_local(0.5, 0.3),
                },
                parameter: 0.5,
                winding,
                active_points: points,
                inactive_points: Vec::new(),
                circle: None,
                arc: None,
                conic_scalar: None,
                conic_vector: None,
                nurbs: None,
            }
        }
        Family::ClampedBSpline
        | Family::PeriodicBSpline
        | Family::ClampedNurbs
        | Family::PeriodicNurbs => {
            add_spline_parent(sketch, family, target, tangent, scale, winding)
        }
    };

    let jet = evaluate_contact(sketch, fixture.contact).unwrap();
    let tolerance = 2.0e-10 * scale.max(1.0);
    assert!(
        (jet.position - target).norm() <= tolerance,
        "{} position: {:?} != {:?}",
        family.label(),
        jet.position,
        target
    );
    assert!(
        jet.first_derivative.normalize().dot(&tangent) >= 1.0 - 2.0e-12,
        "{} tangent: {:?} != {:?}",
        family.label(),
        jet.first_derivative,
        tangent
    );
    assert_eq!(
        fixture.parameter.to_bits(),
        fixture.contact.parameter.to_bits()
    );
    assert_eq!(fixture.winding, winding);
    assert!(matches!(
        fixture.contact.neighborhood,
        CurveContactNeighborhood::Local { lower, upper }
            if lower < fixture.parameter && fixture.parameter < upper
    ));
    fixture
}

#[allow(clippy::too_many_arguments)]
fn add_spline_parent(
    sketch: &mut Sketch,
    family: Family,
    target: Point2<f64>,
    tangent: Vector2<f64>,
    scale: f64,
    winding: i32,
) -> ParentFixture {
    let periodic = matches!(family, Family::PeriodicBSpline | Family::PeriodicNurbs);
    let rational = matches!(family, Family::ClampedNurbs | Family::PeriodicNurbs);
    let base_controls = if periodic {
        vec![
            Point2::new(0.0, 0.0),
            Point2::new(1.5, -0.2),
            Point2::new(2.0, 1.4),
            Point2::new(0.5, 2.2),
            Point2::new(-0.8, 1.0),
        ]
    } else {
        vec![
            Point2::new(-2.0, -0.5),
            Point2::new(-1.2, 1.1),
            Point2::new(-0.3, -0.4),
            Point2::new(0.7, 1.0),
            Point2::new(1.4, -0.7),
            Point2::new(2.2, 0.4),
        ]
    };
    let degree = if periodic { 2 } else { 3 };
    let knots = if periodic {
        vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0]
    } else {
        vec![0.0, 0.0, 0.0, 0.0, 0.34, 0.67, 1.0, 1.0, 1.0, 1.0]
    };
    let weights = if periodic {
        vec![0.8, 1.0, 1.3, 0.7, 1.15]
    } else {
        vec![0.8, 1.0, 1.3, 0.7, 1.15, 0.9]
    };
    let (base_jet, span_ordinal) = if rational {
        let curve = if periodic {
            NurbsCurve2::try_periodic(
                degree,
                base_controls.clone(),
                weights.clone(),
                knots.clone(),
            )
        } else {
            NurbsCurve2::try_clamped(
                degree,
                base_controls.clone(),
                weights.clone(),
                knots.clone(),
            )
        }
        .unwrap();
        let ordinal = if periodic {
            curve.basis().spans().len() - 1
        } else {
            1
        };
        (
            curve
                .jet_on_span(curve.basis().spans()[ordinal].index(), 0.43)
                .unwrap(),
            ordinal,
        )
    } else {
        let curve = if periodic {
            BSplineCurve2::try_periodic(degree, base_controls.clone(), knots.clone())
        } else {
            BSplineCurve2::try_clamped(degree, base_controls.clone(), knots.clone())
        }
        .unwrap();
        let ordinal = if periodic {
            curve.basis().spans().len() - 1
        } else {
            1
        };
        (
            curve
                .jet_on_span(curve.basis().spans()[ordinal].index(), 0.43)
                .unwrap(),
            ordinal,
        )
    };
    let transform = Similarity::new(
        scale,
        base_jet.position,
        base_jet.first_derivative,
        target,
        tangent,
    );
    let controls = add_points(
        sketch,
        family.label(),
        &base_controls
            .iter()
            .map(|point| transform.point(*point))
            .collect::<Vec<_>>(),
    );

    if rational {
        let nurbs = sketch
            .add_named_nurbs(
                family.label(),
                if periodic {
                    BSplineForm::Periodic
                } else {
                    BSplineForm::Clamped
                },
                degree,
                controls.clone(),
                weights,
                1,
                knots,
            )
            .unwrap();
        let span = sketch.nurbs(nurbs).unwrap().basis().spans()[span_ordinal].index();
        let support = sketch
            .nurbs(nurbs)
            .unwrap()
            .basis()
            .span(span)
            .unwrap()
            .support()
            .to_vec();
        let (active_points, inactive_points) = split_support(&controls, &support);
        ParentFixture {
            family,
            contact: SketchCurveContact {
                curve: SketchCurve::Nurbs { nurbs, span },
                parameter: 0.43,
                neighborhood: explicit_local(0.43, 0.25),
            },
            parameter: 0.43,
            winding,
            active_points,
            inactive_points,
            circle: None,
            arc: None,
            conic_scalar: None,
            conic_vector: None,
            nurbs: Some(NurbsIncidence {
                id: nurbs,
                active_support: support,
                gauge_index: 1,
                control_count: controls.len(),
            }),
        }
    } else {
        let spline = sketch
            .add_named_bspline(
                family.label(),
                if periodic {
                    BSplineForm::Periodic
                } else {
                    BSplineForm::Clamped
                },
                degree,
                controls.clone(),
                knots,
            )
            .unwrap();
        let span = sketch.bspline(spline).unwrap().basis().spans()[span_ordinal].index();
        let support = sketch
            .bspline(spline)
            .unwrap()
            .basis()
            .span(span)
            .unwrap()
            .support()
            .to_vec();
        let (active_points, inactive_points) = split_support(&controls, &support);
        ParentFixture {
            family,
            contact: SketchCurveContact {
                curve: SketchCurve::BSpline { spline, span },
                parameter: 0.43,
                neighborhood: explicit_local(0.43, 0.25),
            },
            parameter: 0.43,
            winding,
            active_points,
            inactive_points,
            circle: None,
            arc: None,
            conic_scalar: None,
            conic_vector: None,
            nurbs: None,
        }
    }
}

fn split_support(
    controls: &[geosolve_sketch::PointId],
    support: &[usize],
) -> (Vec<geosolve_sketch::PointId>, Vec<geosolve_sketch::PointId>) {
    controls
        .iter()
        .copied()
        .enumerate()
        .partition_map(|(index, control)| {
            if support.contains(&index) {
                Either::Left(control)
            } else {
                Either::Right(control)
            }
        })
}

enum Either<L, R> {
    Left(L),
    Right(R),
}

trait PartitionMap: Iterator + Sized {
    fn partition_map<L, R>(
        self,
        mut map: impl FnMut(Self::Item) -> Either<L, R>,
    ) -> (Vec<L>, Vec<R>) {
        let mut left = Vec::new();
        let mut right = Vec::new();
        for value in self {
            match map(value) {
                Either::Left(value) => left.push(value),
                Either::Right(value) => right.push(value),
            }
        }
        (left, right)
    }
}

impl<I: Iterator> PartitionMap for I {}

fn evaluate_contact(sketch: &Sketch, contact: SketchCurveContact) -> Result<CurveJet2, String> {
    match contact.curve {
        SketchCurve::Line { segment, domain } => {
            let segment = sketch.segment(segment).ok_or("missing segment")?;
            line_jet(
                sketch
                    .point(segment.start())
                    .ok_or("missing start")?
                    .position(),
                sketch.point(segment.end()).ok_or("missing end")?.position(),
                match domain {
                    LineParameterDomain::SupportingLine => CurveParameterDomain::SupportingLine,
                    LineParameterDomain::BoundedSegment => CurveParameterDomain::Bounded {
                        lower: 0.0,
                        upper: 1.0,
                    },
                },
                contact.parameter,
            )
            .map_err(|error| error.to_string())
        }
        SketchCurve::Circle(circle) => {
            let circle = sketch.circle(circle).ok_or("missing circle")?;
            circle_jet(
                sketch
                    .point(circle.center())
                    .ok_or("missing center")?
                    .position(),
                circle.radius(),
                contact.parameter,
            )
            .map_err(|error| error.to_string())
        }
        SketchCurve::Arc(arc) => {
            let arc = sketch.arc(arc).ok_or("missing arc")?;
            circular_arc_jet(
                sketch
                    .point(arc.center())
                    .ok_or("missing center")?
                    .position(),
                arc.radius(),
                arc.start_angle(),
                arc.signed_sweep(),
                contact.parameter,
            )
            .map_err(|error| error.to_string())
        }
        SketchCurve::Bezier(bezier) => sketch
            .evaluate_bezier(bezier, contact.parameter)
            .map_err(|error| error.to_string()),
        SketchCurve::Conic(conic) => sketch
            .evaluate_conic(conic, contact.parameter)
            .map_err(|error| error.to_string()),
        SketchCurve::BSpline { spline, span } => sketch
            .evaluate_bspline(spline, span, contact.parameter)
            .map_err(|error| error.to_string()),
        SketchCurve::Nurbs { nurbs, span } => sketch
            .evaluate_nurbs(nurbs, span, contact.parameter)
            .map_err(|error| error.to_string()),
    }
}

#[derive(Clone, Copy)]
struct BranchCode {
    first_side: CurveNormalSide,
    second_side: CurveNormalSide,
    order: FilletEndpointOrder,
    sweep: ArcSweep,
}

fn branch_code(code: usize) -> BranchCode {
    BranchCode {
        first_side: if code & 1 == 0 {
            CurveNormalSide::Left
        } else {
            CurveNormalSide::Right
        },
        second_side: if code & 2 == 0 {
            CurveNormalSide::Left
        } else {
            CurveNormalSide::Right
        },
        order: if code & 4 == 0 {
            FilletEndpointOrder::FirstThenSecond
        } else {
            FilletEndpointOrder::SecondThenFirst
        },
        sweep: if code & 8 == 0 {
            ArcSweep::CounterClockwise
        } else {
            ArcSweep::Clockwise
        },
    }
}

fn side_sign(side: CurveNormalSide) -> f64 {
    match side {
        CurveNormalSide::Left => 1.0,
        CurveNormalSide::Right => -1.0,
    }
}

struct FilletFixture {
    sketch: Sketch,
    parents: [ParentFixture; 2],
    arc: geosolve_sketch::ArcId,
    constraint: geosolve_sketch::SketchConstraintId,
    radius_dimension: Option<geosolve_sketch::SketchDimensionId>,
    center: Point2<f64>,
    radius: f64,
    branch: BranchCode,
}

fn fillet_fixture(
    families: [Family; 2],
    scale: f64,
    rotation: f64,
    translation: Vector2<f64>,
    branch: BranchCode,
    radius_mode: Option<DimensionMode>,
) -> FilletFixture {
    let mut sketch = Sketch::new(scale).unwrap();
    let center = Point2::from(translation);
    let radius = scale;
    let tangents = [unit(rotation), unit(rotation + 1.13)];
    let sides = [branch.first_side, branch.second_side];
    let contacts = [0, 1]
        .map(|index| center - left_normal(tangents[index]) * (side_sign(sides[index]) * radius));
    let parents = [
        add_parent(&mut sketch, families[0], contacts[0], tangents[0], scale, 1),
        add_parent(
            &mut sketch,
            families[1],
            contacts[1],
            tangents[1],
            scale,
            -1,
        ),
    ];
    assert_ne!(parents[0].contact.curve, parents[1].contact.curve);

    let ordered = match branch.order {
        FilletEndpointOrder::FirstThenSecond => contacts,
        FilletEndpointOrder::SecondThenFirst => [contacts[1], contacts[0]],
    };
    let angles = ordered.map(|point| {
        let offset = point - center;
        offset.y.atan2(offset.x)
    });
    let center_id = sketch.add_named_point("fillet center", center).unwrap();
    let arc = sketch
        .add_named_arc(
            "fillet output",
            center_id,
            radius,
            angles[0],
            angles[1],
            branch.sweep,
        )
        .unwrap();
    let constraint = sketch
        .add_curve_curve_fillet(
            arc,
            parents[0].contact,
            branch.first_side,
            parents[1].contact,
            branch.second_side,
            branch.order,
        )
        .unwrap();
    let radius_dimension = radius_mode.map(|mode| {
        sketch
            .add_arc_radius(arc, radius, mode)
            .expect("fillet radius dimension must construct")
    });
    FilletFixture {
        sketch,
        parents,
        arc,
        constraint,
        radius_dimension,
        center,
        radius,
        branch,
    }
}

fn assert_geometry(fixture: &FilletFixture, tolerance: f64) {
    let jets = fixture.parents.each_ref().map(|parent| {
        evaluate_contact(&fixture.sketch, parent.contact).expect("parent jet must evaluate")
    });
    for ((jet, parent), side) in jets
        .iter()
        .zip(&fixture.parents)
        .zip([fixture.branch.first_side, fixture.branch.second_side])
    {
        assert_eq!(parent.family.label(), parent.family.label());
        let tangent = jet.first_derivative.normalize();
        let normal = left_normal(tangent);
        let predicted = jet.position + normal * (side_sign(side) * fixture.radius);
        assert!(
            (predicted - fixture.center).norm() <= tolerance,
            "{} center-normal mismatch: {:?} != {:?}",
            parent.family.label(),
            predicted,
            fixture.center
        );
        let radial = jet.position - fixture.center;
        assert!((radial.norm() - fixture.radius).abs() <= tolerance);
        assert!(radial.dot(&tangent).abs() <= tolerance);
    }
    let arc = fixture.sketch.arc(fixture.arc).unwrap();
    let arc_center = fixture.sketch.point(arc.center()).unwrap().position();
    assert!((arc_center - fixture.center).norm() <= tolerance);
    assert!((arc.radius() - fixture.radius).abs() <= tolerance);
    assert_eq!(arc.sweep(), fixture.branch.sweep);
    let endpoints = arc.endpoints(arc_center).unwrap();
    let expected = match fixture.branch.order {
        FilletEndpointOrder::FirstThenSecond => [jets[0].position, jets[1].position],
        FilletEndpointOrder::SecondThenFirst => [jets[1].position, jets[0].position],
    };
    assert!((endpoints.0 - expected[0]).norm() <= tolerance);
    assert!((endpoints.1 - expected[1]).norm() <= tolerance);
}

fn assert_incidence(fixture: &FilletFixture, compiled: &geosolve_sketch::CompiledSketch) {
    let source = compiled
        .source_mappings()
        .iter()
        .find(|mapping| mapping.source == SketchSource::Constraint(fixture.constraint))
        .unwrap();
    assert_eq!(source.residual_ids.len(), 1);
    let residual = compiled.problem().residual(source.residual_ids[0]).unwrap();
    let incident = residual.incident_variables();
    for parent in &fixture.parents {
        for point in &parent.active_points {
            let variable = compiled.variable_for_point(*point).unwrap();
            assert!(
                incident.contains(&variable),
                "{} omitted active control {point:?}",
                parent.family.label()
            );
        }
        for point in &parent.inactive_points {
            let variable = compiled.variable_for_point(*point).unwrap();
            assert!(
                !incident.contains(&variable),
                "{} included inactive control {point:?}",
                parent.family.label()
            );
        }
        if let Some(circle) = parent.circle {
            assert!(incident.contains(&compiled.variable_for_circle_radius(circle).unwrap()));
        }
        if let Some(arc) = parent.arc {
            assert!(incident.contains(&compiled.variable_for_arc_radius(arc).unwrap()));
        }
        if let Some((conic, role)) = parent.conic_scalar {
            assert!(incident.contains(&compiled.variable_for_conic_scalar(conic, role).unwrap()));
        }
        if let Some((conic, role)) = parent.conic_vector {
            assert!(incident.contains(&compiled.variable_for_conic_vector(conic, role).unwrap()));
        }
        if let Some(nurbs) = &parent.nurbs {
            assert!(
                compiled
                    .variable_for_nurbs_weight(nurbs.id, nurbs.gauge_index)
                    .is_none(),
                "NURBS gauge became a solver coordinate"
            );
            for index in 0..nurbs.control_count {
                if index == nurbs.gauge_index {
                    continue;
                }
                let variable = compiled.variable_for_nurbs_weight(nurbs.id, index).unwrap();
                assert_eq!(
                    incident.contains(&variable),
                    nurbs.active_support.contains(&index),
                    "{} weight {index} has incorrect incidence",
                    parent.family.label()
                );
            }
        }
    }
}

fn assert_audit_and_jacobians(fixture: &FilletFixture, compiled: &geosolve_sketch::CompiledSketch) {
    let source = compiled
        .source_mappings()
        .iter()
        .find(|mapping| mapping.source == SketchSource::Constraint(fixture.constraint))
        .unwrap();
    let residual_id = source.residual_ids[0];
    let descriptors = compiled
        .problem()
        .audit_rows()
        .unwrap()
        .into_iter()
        .filter(|row| row.residual_id == residual_id)
        .collect::<Vec<_>>();
    assert_eq!(descriptors.len(), 6);
    for (index, row) in descriptors.iter().enumerate() {
        assert_eq!(row.row_in_block, index);
        assert!(!row.template.is_empty());
        assert!(!row.bindings.is_empty());
        assert!(!row.unit.is_empty());
        assert!(row.scale.is_finite() && row.scale > 0.0);
    }
    let rows = compiled
        .problem()
        .audit_snapshot()
        .unwrap()
        .sources
        .into_iter()
        .flat_map(|source| source.rows)
        .filter(|row| row.residual_id == residual_id)
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 6);
    assert!(rows.iter().all(|row| {
        row.evaluation_status == AuditEvaluationStatus::Evaluated
            && row.raw_residual.is_finite()
            && row.normalized_residual.is_finite()
            && row.normalized_residual.abs() <= 2.0e-10
    }));

    let report = compiled.problem().check_jacobians(1.0e-6).unwrap();
    assert!(
        report.blocks.iter().all(|block| {
            block.max_relative_error <= 1.0e-6 || block.max_absolute_error <= 1.0e-8
        }),
        "{} / {} max relative={:e}, absolute={:e}: {report:#?}",
        fixture.parents[0].family.label(),
        fixture.parents[1].family.label(),
        report.max_relative_error(),
        report.max_absolute_error()
    );
}

#[test]
fn all_fourteen_families_and_105_unordered_pairs_use_one_finite_fillet_residual() {
    assert_eq!(FAMILIES.len(), 14);
    let mut pair_count = 0;
    for (first_index, first) in FAMILIES.iter().copied().enumerate() {
        for second in FAMILIES[first_index..].iter().copied() {
            pair_count += 1;
            let fixture = fillet_fixture(
                [first, second],
                1.0,
                0.0,
                Vector2::zeros(),
                branch_code(0),
                None,
            );
            let compiled = fixture
                .sketch
                .compile(SketchSolveRequest::default().without_previous_state_preferences())
                .unwrap();
            assert_geometry(&fixture, 2.0e-10);
            assert_incidence(&fixture, &compiled);
            assert_audit_and_jacobians(&fixture, &compiled);
        }
    }
    assert_eq!(pair_count, 14 * 15 / 2);
}

#[test]
fn transformed_scaled_solve_cover_hits_every_family_role_and_all_branch_codes() {
    let rotation = 0.37;
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let translation = Vector2::new(7.0 * scale, -3.0 * scale);
        let mut first_roles = [false; 14];
        let mut second_roles = [false; 14];
        for code in 0..16 {
            let first = code % FAMILIES.len();
            let second = (code + 7) % FAMILIES.len();
            first_roles[first] = true;
            second_roles[second] = true;
            let mode = if code & 1 == 0 {
                DimensionMode::Driving
            } else {
                DimensionMode::Reference
            };
            let mut fixture = fillet_fixture(
                [FAMILIES[first], FAMILIES[second]],
                scale,
                rotation,
                translation,
                branch_code(code),
                Some(mode),
            );
            let result = fixture
                .sketch
                .solve(SketchSolveRequest::default(), SolverConfig::default())
                .unwrap();
            assert!(
                result.accepted(),
                "scale={scale:e}, code={code}, {} / {}: {:?}",
                FAMILIES[first].label(),
                FAMILIES[second].label(),
                result.rejection
            );
            assert_eq!(result.core_report.hard_validity, HardValidity::Valid);
            assert!(result.acceptance_hard_residual_max.unwrap() <= 1.0e-9);
            assert!(result.core_report.hard_residuals_validated);
            assert_geometry(&fixture, 3.0e-8 * scale);
            if mode == DimensionMode::Reference {
                let dimension = fixture.radius_dimension.unwrap();
                let measured = result
                    .reference_values
                    .iter()
                    .find(|value| value.dimension_id == dimension)
                    .unwrap()
                    .value;
                assert!((measured - scale).abs() <= 2.0e-9 * scale);
            }
        }
        assert!(first_roles.into_iter().all(|covered| covered));
        assert!(second_roles.into_iter().all(|covered| covered));

        let families = [Family::ClampedBSpline, Family::PeriodicNurbs];
        let mut driving = fillet_fixture(
            families,
            scale,
            rotation,
            translation,
            branch_code(5),
            Some(DimensionMode::Driving),
        );
        let driving = driving
            .sketch
            .solve(SketchSolveRequest::default(), SolverConfig::default())
            .unwrap();
        let mut reference = fillet_fixture(
            families,
            scale,
            rotation,
            translation,
            branch_code(5),
            Some(DimensionMode::Reference),
        );
        let reference = reference
            .sketch
            .solve(SketchSolveRequest::default(), SolverConfig::default())
            .unwrap();
        assert!(driving.accepted() && reference.accepted());
        assert_eq!(
            reference.core_report.local_degrees_of_freedom,
            driving.core_report.local_degrees_of_freedom + 1
        );
    }
}

#[test]
fn polyline_span_is_exactly_one_line_alias_in_generic_fillet_lowering() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let line_points = [
        document.add_point("line start", [-2.0, -1.0]).unwrap(),
        document.add_point("line end", [2.0, -1.0]).unwrap(),
    ];
    let line = document
        .add_curve(
            "line",
            CurveDefinition::Line {
                start: line_points[0],
                end: line_points[1],
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let polyline_points = [
        document.add_point("polyline start", [1.0, -2.0]).unwrap(),
        document.add_point("polyline end", [1.0, 2.0]).unwrap(),
    ];
    let polyline = document
        .add_curve(
            "polyline",
            CurveDefinition::Polyline {
                points: polyline_points.to_vec(),
                closed: false,
                branch_directions: vec![[0.0, 1.0]],
            },
        )
        .unwrap();
    let ids = document
        .add_curve_curve_fillet(
            "line-polyline",
            CurveCurveFilletRequest {
                first: document_parent(CurveSpan::line(line), 0.5),
                second: document_parent(
                    CurveSpan {
                        curve: polyline,
                        segment: 0,
                    },
                    0.5,
                ),
                endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
                sweep: DocumentArcSweep::CounterClockwise,
                radius: 1.0,
                radius_mode: DocumentDimensionMode::Driving,
            },
        )
        .unwrap();
    let lowered = document.lower().unwrap();
    assert!(matches!(
        lowered.mappings().runtime_curve(line),
        Some(RuntimeCurve::Line(_))
    ));
    assert!(matches!(
        lowered.mappings().runtime_curve(polyline),
        Some(RuntimeCurve::Polyline(segments)) if segments.len() == 1
    ));
    let source = document.constraint(ids.constraint).unwrap().source_id;
    let RuntimeSource::Constraint(runtime) = lowered.mappings().runtime_source(source).unwrap()
    else {
        panic!("generic fillet must lower to a runtime constraint")
    };
    assert!(matches!(
        lowered.sketch().constraint(runtime).unwrap().kind(),
        geosolve_sketch::SketchConstraintKind::CurveCurveFillet {
            first: SketchCurveContact {
                curve: SketchCurve::Line { .. },
                ..
            },
            second: SketchCurveContact {
                curve: SketchCurve::Line { .. },
                ..
            },
            ..
        }
    ));
}

fn document_parent(curve: CurveSpan, parameter: f64) -> CurveFilletParentRequest {
    CurveFilletParentRequest {
        curve,
        parameter,
        winding: 0,
        neighborhood: geosolve_sketch::ContactNeighborhood::Local {
            lower: parameter - 0.3,
            upper: parameter + 0.3,
        },
        side: DocumentCurveNormalSide::Left,
        trim_endpoint: DocumentFilletTrimEndpoint::Start,
        periodic_anchor: None,
    }
}

#[test]
fn singular_pole_zero_speed_and_nonfinite_seeds_reject_without_partial_state() {
    for radius in [1.0, 1.0 - 5.0e-9] {
        let mut sketch = offset_singularity_fixture(radius);
        let result = sketch
            .solve(SketchSolveRequest::default(), SolverConfig::default())
            .unwrap();
        assert!(!result.accepted(), "radius={radius}: {result:#?}");
        assert!(result.geometry.arcs.iter().all(|arc| {
            arc.center.coords.iter().all(|value| value.is_finite()) && arc.radius.is_finite()
        }));
    }

    let mut zero_speed = Sketch::new(1.0).unwrap();
    let controls = add_points(
        &mut zero_speed,
        "zero-speed cubic",
        &[
            Point2::new(-1.0, 0.0),
            Point2::new(1.0, 0.0),
            Point2::new(1.0, 0.0),
            Point2::new(-1.0, 0.0),
        ],
    );
    let cubic = zero_speed
        .add_cubic_bezier(
            "zero-speed cubic",
            [controls[0], controls[1], controls[2], controls[3]],
        )
        .unwrap();
    let line_start = zero_speed.add_point(Point2::new(0.0, -2.0)).unwrap();
    let line_end = zero_speed.add_point(Point2::new(0.0, 2.0)).unwrap();
    let line = zero_speed.add_segment(line_start, line_end).unwrap();
    let center = zero_speed.add_point(Point2::origin()).unwrap();
    let arc = zero_speed
        .add_arc(center, 1.0, 0.0, FRAC_PI_2, ArcSweep::CounterClockwise)
        .unwrap();
    let before = zero_speed.constraints().count();
    assert!(
        zero_speed
            .add_curve_curve_fillet(
                arc,
                SketchCurveContact {
                    curve: SketchCurve::Bezier(cubic),
                    parameter: 0.5,
                    neighborhood: explicit_local(0.5, 0.2),
                },
                CurveNormalSide::Left,
                SketchCurveContact {
                    curve: SketchCurve::Line {
                        segment: line,
                        domain: LineParameterDomain::BoundedSegment,
                    },
                    parameter: 0.5,
                    neighborhood: explicit_local(0.5, 0.2),
                },
                CurveNormalSide::Left,
                FilletEndpointOrder::FirstThenSecond,
            )
            .is_err()
    );
    assert_eq!(zero_speed.constraints().count(), before);

    let mut rational = Sketch::new(1.0).unwrap();
    let start = rational.add_point(Point2::new(-1.0, 0.0)).unwrap();
    let end = rational.add_point(Point2::new(1.0, 0.0)).unwrap();
    assert!(
        rational
            .add_rational_quadratic(start, Vector2::new(0.0, 1.0), -1.0, end)
            .is_err()
    );
    assert_eq!(rational.conics().count(), 0);
    assert!(
        rational
            .add_rational_quadratic(start, Vector2::new(0.0, 1.0), -1.0 + f64::EPSILON, end,)
            .is_err(),
        "the scale-aware near-pole band must reject before a fillet can consume the curve"
    );
    assert_eq!(rational.conics().count(), 0);

    let mut finite = fillet_fixture(
        [Family::Line, Family::CubicBezier],
        1.0,
        0.0,
        Vector2::zeros(),
        branch_code(0),
        None,
    );
    let malformed_center = finite.sketch.add_point(Point2::new(3.0, 3.0)).unwrap();
    let malformed_arc = finite
        .sketch
        .add_arc(
            malformed_center,
            1.0,
            0.0,
            FRAC_PI_2,
            ArcSweep::CounterClockwise,
        )
        .unwrap();
    let before = finite.sketch.constraints().count();
    let mut malformed = finite.parents[0].contact;
    malformed.parameter = f64::NAN;
    assert!(
        finite
            .sketch
            .add_curve_curve_fillet(
                malformed_arc,
                malformed,
                CurveNormalSide::Left,
                finite.parents[1].contact,
                CurveNormalSide::Left,
                FilletEndpointOrder::FirstThenSecond,
            )
            .is_err()
    );
    assert_eq!(finite.sketch.constraints().count(), before);
}

fn offset_singularity_fixture(radius: f64) -> Sketch {
    let mut sketch = Sketch::new(1.0).unwrap();
    let circle_center = sketch.add_point(Point2::origin()).unwrap();
    let circle = sketch.add_circle(circle_center, 1.0).unwrap();
    let center = Point2::new(1.0 - radius, 0.0);
    let line_start = sketch.add_point(Point2::new(-2.0, -radius)).unwrap();
    let line_end = sketch.add_point(Point2::new(2.0, -radius)).unwrap();
    let line = sketch.add_segment(line_start, line_end).unwrap();
    let fillet_center = sketch.add_point(center).unwrap();
    let arc = sketch
        .add_arc(fillet_center, radius, 0.0, -FRAC_PI_2, ArcSweep::Clockwise)
        .unwrap();
    sketch
        .add_curve_curve_fillet(
            arc,
            SketchCurveContact {
                curve: SketchCurve::Circle(circle),
                parameter: 0.0,
                neighborhood: explicit_local(0.0, 0.2),
            },
            CurveNormalSide::Left,
            SketchCurveContact {
                curve: SketchCurve::Line {
                    segment: line,
                    domain: LineParameterDomain::BoundedSegment,
                },
                parameter: (center.x + 2.0) / 4.0,
                neighborhood: explicit_local((center.x + 2.0) / 4.0, 0.2),
            },
            CurveNormalSide::Left,
            FilletEndpointOrder::FirstThenSecond,
        )
        .unwrap();
    sketch
        .add_arc_radius(arc, radius, DimensionMode::Driving)
        .unwrap();
    sketch
}

#[test]
fn bounded_line_cubic_trim_updates_persists_and_rolls_back_escape_atomically() {
    let (document, line, cubic) = line_cubic_document();
    let definitions = [
        document.curve(line).unwrap().definition.clone(),
        document.curve(cubic).unwrap().definition.clone(),
    ];
    let mut session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let request = CurveCurveFilletRequest {
        first: document_parent(CurveSpan::line(line), 0.5),
        second: document_parent(CurveSpan::line(cubic), 0.5),
        endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
        sweep: DocumentArcSweep::CounterClockwise,
        radius: 1.0,
        radius_mode: DocumentDimensionMode::Driving,
    };
    let created = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::CreateCurveCurveFillet {
                label: "line-cubic trim".into(),
                request,
            },
        ))
        .unwrap();
    assert!(created.accepted(), "{created:#?}");
    let DocumentCommandEffect::CreatedCurveCurveFillet(ids) = created.effect.unwrap() else {
        panic!("expected generic fillet creation")
    };
    let before_intervals = [line, cubic].map(|curve| {
        session
            .document()
            .visible_interval(CurveSpan::line(curve))
            .unwrap()
    });
    let created_json = session.export_json().unwrap();
    assert_eq!(session.history_len(), 1);
    assert_eq!(
        session.document().curve(line).unwrap().definition,
        definitions[0]
    );
    assert_eq!(
        session.document().curve(cubic).unwrap().definition,
        definitions[1]
    );

    let edited = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetScalarValue {
                scalar: ids.radius_target,
                value: 0.8,
            },
        ))
        .unwrap();
    assert!(edited.accepted(), "{edited:#?}");
    let after_intervals = [line, cubic].map(|curve| {
        session
            .document()
            .visible_interval(CurveSpan::line(curve))
            .unwrap()
    });
    assert!(
        before_intervals
            .iter()
            .zip(after_intervals.iter())
            .all(|(before, after)| {
                (before.start - after.start).abs() > 1.0e-5
                    || (before.end - after.end).abs() > 1.0e-5
            })
    );
    assert_eq!(
        session.document().curve(line).unwrap().definition,
        definitions[0]
    );
    assert_eq!(
        session.document().curve(cubic).unwrap().definition,
        definitions[1]
    );
    let edited_json = session.export_json().unwrap();
    assert_eq!(
        SketchDocument::from_json(&edited_json)
            .unwrap()
            .to_canonical_json()
            .unwrap(),
        edited_json
    );
    session.undo(session.revision()).unwrap();
    assert_eq!(session.export_json().unwrap(), created_json);
    session.redo(session.revision()).unwrap();
    assert_eq!(session.export_json().unwrap(), edited_json);

    let retained_revision = session.revision();
    let retained_history = session.history_len();
    for value in [10.0, f64::NAN] {
        let rejected = session.apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetScalarValue {
                scalar: ids.radius_target,
                value,
            },
        ));
        assert!(rejected.is_err() || !rejected.unwrap().accepted());
        assert_eq!(session.revision(), retained_revision);
        assert_eq!(session.history_len(), retained_history);
        assert_eq!(session.export_json().unwrap(), edited_json);
    }
}

fn line_cubic_document() -> (
    SketchDocument,
    geosolve_sketch::CurveId,
    geosolve_sketch::CurveId,
) {
    let mut document = SketchDocument::new(1.0).unwrap();
    let line_points = [
        document.add_point("line start", [-2.0, -1.0]).unwrap(),
        document.add_point("line end", [2.0, -1.0]).unwrap(),
    ];
    let line = document
        .add_curve(
            "line",
            CurveDefinition::Line {
                start: line_points[0],
                end: line_points[1],
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let target = Point2::new(1.0, 0.0);
    let tangent = Vector2::y();
    let local = [[-1.5, -1.5], [-0.5, 0.5], [0.5, 0.5], [1.5, -1.5]];
    let controls = local.map(|point| {
        let position = map_local(target, tangent, 1.0, point);
        document
            .add_point("cubic control", [position.x, position.y])
            .unwrap()
    });
    let cubic = document
        .add_curve("cubic", CurveDefinition::CubicBezier { controls })
        .unwrap();
    for point in line_points.into_iter().chain(controls) {
        let target = document.point(point).unwrap().position;
        document
            .add_constraint(
                "fixed support",
                DocumentConstraintDefinition::FixedPoint { point, target },
            )
            .unwrap();
    }
    (document, line, cubic)
}

#[test]
fn periodic_bspline_trim_preserves_nonzero_contact_and_boundary_winding() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let controls = [[0.0, 0.0], [1.5, -0.2], [2.0, 1.4], [0.5, 2.2], [-0.8, 1.0]]
        .map(|position| document.add_point("periodic control", position).unwrap());
    let spline = document
        .add_curve(
            "periodic parent",
            CurveDefinition::BSpline {
                form: DocumentBSplineForm::Periodic,
                degree: 2,
                controls: controls.to_vec(),
                knots: vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
                span_ids: vec![11, 17, 23, 29, 31],
                next_span_id: 32,
            },
        )
        .unwrap();
    let spline_span = CurveSpan {
        curve: spline,
        segment: 31,
    };
    let spline_jet = document.evaluate_curve_jet(spline_span, 0.3).unwrap();
    let spline_differential = spline_jet.differential().unwrap();
    let radius = 0.2;
    let center = spline_jet.position + spline_differential.left_normal * radius;
    let line_tangent = spline_differential.left_normal;
    let line_normal = left_normal(line_tangent);
    let line_contact = center - line_normal * radius;
    let line_points = [
        line_contact - line_tangent * 2.0,
        line_contact + line_tangent * 2.0,
    ]
    .map(|position| {
        document
            .add_point("line control", [position.x, position.y])
            .unwrap()
    });
    let line = document
        .add_curve(
            "line parent",
            CurveDefinition::Line {
                start: line_points[0],
                end: line_points[1],
                branch_direction: [line_tangent.x, line_tangent.y],
            },
        )
        .unwrap();
    let ids = document
        .add_curve_curve_fillet(
            "wound periodic spline fillet",
            CurveCurveFilletRequest {
                first: CurveFilletParentRequest {
                    curve: spline_span,
                    parameter: 0.3,
                    winding: 2,
                    neighborhood: ContactNeighborhood::Local {
                        lower: 0.1,
                        upper: 0.5,
                    },
                    side: DocumentCurveNormalSide::Left,
                    trim_endpoint: DocumentFilletTrimEndpoint::End,
                    periodic_anchor: None,
                },
                second: CurveFilletParentRequest {
                    curve: CurveSpan::line(line),
                    parameter: 0.5,
                    winding: 0,
                    neighborhood: ContactNeighborhood::Local {
                        lower: 0.2,
                        upper: 0.8,
                    },
                    side: DocumentCurveNormalSide::Left,
                    trim_endpoint: DocumentFilletTrimEndpoint::Start,
                    periodic_anchor: None,
                },
                endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
                sweep: DocumentArcSweep::CounterClockwise,
                radius,
                radius_mode: DocumentDimensionMode::Driving,
            },
        )
        .unwrap();
    assert_eq!(document.contact(ids.contacts[0]).unwrap().winding, 2);
    let view = document.trim_view(spline_span).unwrap();
    assert!(matches!(
        view.start,
        geosolve_sketch::DocumentTrimBoundary::Fixed(
            geosolve_sketch::DocumentTrimParameter {
                parameter,
                winding: 2
            }
        ) if parameter.to_bits() == 0.0f64.to_bits()
    ));
    let interval = document.visible_interval(spline_span).unwrap();
    assert_eq!(interval.start.to_bits(), 0.0f64.to_bits());
    assert!((interval.end - 0.3).abs() <= 1.0e-12);
    let canonical = document.to_canonical_json().unwrap();
    let migrated = SketchDocument::from_json(&canonical).unwrap();
    assert_eq!(migrated.to_canonical_json().unwrap(), canonical);
    assert_eq!(migrated.contact(ids.contacts[0]).unwrap().winding, 2);
    let session = SketchDocumentSession::new(
        migrated,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    assert!(session.accepted_result().accepted());
}

#[test]
fn bspline_nurbs_span_trims_preserve_support_history_and_block_refinement() {
    let (document, bspline, nurbs, selected) = spline_trim_document();
    let definitions = [
        document.curve(bspline).unwrap().definition.clone(),
        document.curve(nurbs).unwrap().definition.clone(),
    ];
    let mut session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let created = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::CreateCurveCurveFillet {
                label: "spline span trim".into(),
                request: CurveCurveFilletRequest {
                    first: document_parent(selected[0], 0.43),
                    second: document_parent(selected[1], 0.43),
                    endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
                    sweep: DocumentArcSweep::CounterClockwise,
                    radius: 1.0,
                    radius_mode: DocumentDimensionMode::Driving,
                },
            },
        ))
        .unwrap();
    assert!(created.accepted(), "{created:#?}");
    assert_eq!(session.document().trim_views().len(), 2);
    for (curve, support) in [(bspline, selected[0]), (nurbs, selected[1])] {
        assert!(session.document().trim_view(support).is_some());
        assert_eq!(
            session
                .document()
                .curve_spans(curve)
                .unwrap()
                .into_iter()
                .filter(|span| session.document().trim_view(*span).is_some())
                .collect::<Vec<_>>(),
            vec![support]
        );
    }
    assert_eq!(
        session.document().curve(bspline).unwrap().definition,
        definitions[0]
    );
    assert_eq!(
        session.document().curve(nurbs).unwrap().definition,
        definitions[1]
    );
    let accepted = session.export_json().unwrap();
    assert_eq!(
        SketchDocument::from_json(&accepted)
            .unwrap()
            .to_canonical_json()
            .unwrap(),
        accepted
    );
    let history = session.history_len();
    for edit in [
        DocumentEdit::InsertBSplineKnot {
            curve: bspline,
            parameter: 0.5,
        },
        DocumentEdit::InsertNurbsKnot {
            curve: nurbs,
            parameter: 0.5,
        },
    ] {
        assert!(
            session
                .apply(DocumentCommand::new(session.revision(), edit))
                .is_err()
        );
        assert_eq!(session.export_json().unwrap(), accepted);
        assert_eq!(session.history_len(), history);
    }
    session.undo(session.revision()).unwrap();
    assert!(session.document().trim_views().is_empty());
    session.redo(session.revision()).unwrap();
    assert_eq!(session.export_json().unwrap(), accepted);
}

fn spline_trim_document() -> (
    SketchDocument,
    geosolve_sketch::CurveId,
    geosolve_sketch::CurveId,
    [CurveSpan; 2],
) {
    let mut document = SketchDocument::new(1.0).unwrap();
    let degree = 2;
    let knots = vec![0.0, 0.0, 0.0, 0.34, 0.67, 1.0, 1.0, 1.0];
    let base = vec![
        Point2::new(-2.0, -0.5),
        Point2::new(-1.2, 1.1),
        Point2::new(-0.3, -0.4),
        Point2::new(0.7, 1.0),
        Point2::new(1.4, -0.7),
    ];
    let bspline_geometry = BSplineCurve2::try_clamped(degree, base.clone(), knots.clone()).unwrap();
    let bspline_span = bspline_geometry.basis().spans()[1].index();
    let jet = bspline_geometry.jet_on_span(bspline_span, 0.43).unwrap();
    let transform = Similarity::new(
        1.0,
        jet.position,
        jet.first_derivative,
        Point2::new(0.0, -1.0),
        Vector2::x(),
    );
    let bspline_controls = base
        .iter()
        .map(|point| {
            let point = transform.point(*point);
            document
                .add_point("B-spline control", [point.x, point.y])
                .unwrap()
        })
        .collect::<Vec<_>>();
    let bspline = document
        .add_curve(
            "clamped B-spline",
            CurveDefinition::BSpline {
                form: DocumentBSplineForm::Clamped,
                degree,
                controls: bspline_controls,
                knots: knots.clone(),
                span_ids: vec![41, 73, 89],
                next_span_id: 90,
            },
        )
        .unwrap();

    let weights = vec![0.8, 1.0, 1.3, 0.7, 1.15];
    let nurbs_geometry =
        NurbsCurve2::try_clamped(degree, base.clone(), weights.clone(), knots.clone()).unwrap();
    let nurbs_span = nurbs_geometry.basis().spans()[1].index();
    let jet = nurbs_geometry.jet_on_span(nurbs_span, 0.43).unwrap();
    let transform = Similarity::new(
        1.0,
        jet.position,
        jet.first_derivative,
        Point2::new(1.0, 0.0),
        Vector2::y(),
    );
    let nurbs_controls = base
        .iter()
        .map(|point| {
            let point = transform.point(*point);
            document
                .add_point("NURBS control", [point.x, point.y])
                .unwrap()
        })
        .collect::<Vec<_>>();
    let weight_ids = weights
        .into_iter()
        .map(|weight| {
            document
                .add_scalar(
                    "NURBS weight",
                    weight,
                    ScalarUnit::Parameter,
                    ScalarDomain::Positive,
                )
                .unwrap()
        })
        .collect::<Vec<_>>();
    let nurbs = document
        .add_curve(
            "clamped NURBS",
            CurveDefinition::Nurbs {
                form: DocumentBSplineForm::Clamped,
                degree,
                controls: nurbs_controls,
                weights: weight_ids.clone(),
                gauge_weight: weight_ids[1],
                knots,
                span_ids: vec![11, 17, 23],
                next_span_id: 24,
            },
        )
        .unwrap();
    (
        document,
        bspline,
        nurbs,
        [
            CurveSpan {
                curve: bspline,
                segment: 73,
            },
            CurveSpan {
                curve: nurbs,
                segment: 17,
            },
        ],
    )
}
