// SPDX-License-Identifier: GPL-3.0-or-later
//! The golden fixture matrix: a fixed document whose operands are derived from
//! a [`Family`] and [`FuzzVariant`], then perturbed by the variant's
//! transform. This is the same document the golden authoring oracle uses; it is
//! the shared surface that the fuzzing front-ends perturb.

use geosolve_constraint_editor::{AuthoringOperand, SelectionItem};
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DesignPointId, DesignScalarId, DocumentCoordinateAxis, DocumentId,
    PersistentId, SketchDatum, SketchDocument,
};

use crate::{Family, FuzzVariant};

/// An affine transform applied to every fixture point.
#[derive(Clone, Copy, Debug)]
struct Transform {
    translation: [f64; 2],
    scale: f64,
    cosine: f64,
    sine: f64,
}

impl Transform {
    fn new(variant: FuzzVariant) -> Self {
        Self {
            translation: variant.translation,
            scale: variant.scale,
            cosine: variant.rotation.cos(),
            sine: variant.rotation.sin(),
        }
    }

    fn point(self, value: [f64; 2]) -> [f64; 2] {
        [
            self.translation[0] + self.scale * value[0].mul_add(self.cosine, -value[1] * self.sine),
            self.translation[1] + self.scale * value[0].mul_add(self.sine, value[1] * self.cosine),
        ]
    }
}

/// A fully-built fixture document together with the handles the authoring path
/// needs to reference it.
#[derive(Clone, Debug)]
pub struct MatrixFixture {
    document: SketchDocument,
    // The perturbation applied while building `document`. The survey reads actual
    // document coordinates rather than recomputing from it, so it is retained only
    // for parity with the golden fixture, which uses it to derive expected values.
    #[allow(dead_code)]
    transform: Transform,
    points: [DesignPointId; 6],
    coincident: [DesignPointId; 2],
    contact_point: DesignPointId,
    line_midpoint: DesignPointId,
    lines: [CurveSpan; 2],
    circles: [CurveSpan; 2],
    // Radius scalar handles for the two circles; the survey reads the scalars
    // directly from the document instead, so the handles are retained for parity.
    #[allow(dead_code)]
    circle_radii: [DesignScalarId; 2],
    beziers: [CurveSpan; 2],
    overlapping_line: CurveSpan,
    radial_line: CurveSpan,
    horizontal_contact_parameter: f64,
    overlap_contact_parameter: f64,
    radial_parameter: f64,
    original_points: Vec<(DesignPointId, [f64; 2])>,
    original_scalars: Vec<(DesignScalarId, f64)>,
    pre_satisfied: bool,
}

impl MatrixFixture {
    /// Build the fixture document for a family + variant.
    ///
    /// # Panics
    ///
    /// Panics if the initial sketch document fails to build.
    pub fn new(subject: Family, variant: FuzzVariant) -> Self {
        let transform = Transform::new(variant);
        let displaced_kind = match subject {
            Family::Constraint { kind, .. } if variant.displaced => Some(kind),
            Family::Constraint { .. } | Family::Dimension(_) => None,
        };
        let displaced = |kind| displaced_kind == Some(kind);
        let mut document = SketchDocument::with_id(
            10.0 * variant.scale,
            DocumentId(PersistentId::from_u128(0x4d37_3042_4831_4155_5448_4f52)),
        )
        .expect("oracle document");
        let p = variant.contact_parameter;
        let intersection_x = -2.0 + 4.0 * p;
        let primary_end = [
            2.0,
            if displaced(geosolve_constraint_editor::ResolvedConstraintKind::HorizontalLine)
                || displaced(geosolve_constraint_editor::ResolvedConstraintKind::HorizontalPoints)
            {
                0.3
            } else {
                0.0
            },
        ];
        let cross_x_offset =
            if displaced(geosolve_constraint_editor::ResolvedConstraintKind::VerticalLine)
                || displaced(geosolve_constraint_editor::ResolvedConstraintKind::VerticalPoints)
                || displaced(geosolve_constraint_editor::ResolvedConstraintKind::PerpendicularLines)
            {
                0.3
            } else {
                0.0
            };
        let cross_y_offset =
            if displaced(geosolve_constraint_editor::ResolvedConstraintKind::CurveContact) {
                0.2
            } else {
                0.0
            };
        let symmetric_y_offset =
            if displaced(geosolve_constraint_editor::ResolvedConstraintKind::SymmetricAboutLine)
                || displaced(
                    geosolve_constraint_editor::ResolvedConstraintKind::SymmetricAboutDatumAxis,
                )
            {
                0.3
            } else {
                0.0
            };
        let points = [
            add_point(&mut document, transform, "a", [-2.0, 0.0]),
            add_point(&mut document, transform, "b", primary_end),
            add_point(
                &mut document,
                transform,
                "c",
                [intersection_x, -2.0 + cross_y_offset],
            ),
            add_point(
                &mut document,
                transform,
                "d",
                [intersection_x + cross_x_offset, 2.0 + cross_y_offset],
            ),
            add_point(&mut document, transform, "e", [intersection_x - 3.0, 3.0]),
            add_point(
                &mut document,
                transform,
                "f",
                if matches!(
                    subject,
                    Family::Constraint {
                        kind: geosolve_constraint_editor::ResolvedConstraintKind::ConcentricCurves,
                        ..
                    }
                ) {
                    [
                        intersection_x - 3.0
                            + if displaced(
                                geosolve_constraint_editor::ResolvedConstraintKind::ConcentricCurves,
                            ) {
                                0.25
                            } else {
                                0.0
                            },
                        3.0,
                    ]
                } else {
                    [intersection_x + 3.0, 3.0 + symmetric_y_offset]
                },
            ),
        ];
        let first_line = add_line_from_points(
            &mut document,
            "primary line",
            points[0],
            points[1],
            variant.reverse_spans,
        );
        let second_line = add_line_from_points(
            &mut document,
            "cross line",
            points[2],
            points[3],
            variant.reverse_spans,
        );
        let first_radius = document
            .add_scalar(
                "first radius",
                variant.scale,
                geosolve_sketch::ScalarUnit::Length,
                geosolve_sketch::ScalarDomain::Positive,
            )
            .expect("first radius");
        let first_circle = CurveSpan::line(
            document
                .add_curve(
                    "first circle",
                    CurveDefinition::Circle {
                        center: points[4],
                        radius: first_radius,
                    },
                )
                .expect("first circle"),
        );
        let second_radius = document
            .add_scalar(
                "second radius",
                variant.scale
                    * if displaced(geosolve_constraint_editor::ResolvedConstraintKind::EqualRadius)
                    {
                        1.2
                    } else {
                        1.0
                    },
                geosolve_sketch::ScalarUnit::Length,
                geosolve_sketch::ScalarDomain::Positive,
            )
            .expect("second radius");
        let second_circle = CurveSpan::line(
            document
                .add_curve(
                    "second circle",
                    CurveDefinition::Circle {
                        center: points[5],
                        radius: second_radius,
                    },
                )
                .expect("second circle"),
        );
        let contact_point = add_point(
            &mut document,
            transform,
            "contact point",
            [
                intersection_x,
                if displaced(geosolve_constraint_editor::ResolvedConstraintKind::PointOnCurve) {
                    0.25
                } else {
                    0.0
                },
            ],
        );
        let line_midpoint = add_point(
            &mut document,
            transform,
            "line midpoint",
            [
                0.0,
                if displaced(geosolve_constraint_editor::ResolvedConstraintKind::Midpoint) {
                    0.25
                } else {
                    0.0
                },
            ],
        );
        let coincident = [
            add_point(&mut document, transform, "coincident a", [5.0, 5.0]),
            add_point(
                &mut document,
                transform,
                "coincident b",
                [
                    5.0 + if displaced(
                        geosolve_constraint_editor::ResolvedConstraintKind::CoincidentPoints,
                    ) {
                        0.25
                    } else {
                        0.0
                    },
                    5.0,
                ],
            ),
        ];
        let first_bezier_controls = [
            add_point(&mut document, transform, "bezier 1 start", [-4.0, -4.0]),
            add_point(&mut document, transform, "bezier 1 middle", [-2.0, -2.0]),
            add_point(&mut document, transform, "bezier seam", [0.0, -4.0]),
        ];
        let second_bezier_start =
            if displaced(geosolve_constraint_editor::ResolvedConstraintKind::EndpointContinuity) {
                add_point(
                    &mut document,
                    transform,
                    "displaced bezier 2 start",
                    [0.2, -4.1],
                )
            } else {
                first_bezier_controls[2]
            };
        let endpoint_continuity = matches!(
            subject,
            Family::Constraint {
                kind: geosolve_constraint_editor::ResolvedConstraintKind::EndpointContinuity,
                ..
            }
        );
        let parametric_c2 = endpoint_continuity
            && matches!(
                continuity_option(variant),
                geosolve_sketch::DocumentCurveContinuity::ParametricC2 { .. }
            );
        let second_end = if parametric_c2 {
            [2.0, -7.0]
        } else if endpoint_continuity
            && continuity_option(variant) == geosolve_sketch::DocumentCurveContinuity::G2
        {
            [4.0, -12.0]
        } else {
            [4.0, -4.0]
        };
        let second_middle = if parametric_c2 {
            [1.0, -5.0]
        } else {
            [
                2.0,
                if displaced(geosolve_constraint_editor::ResolvedConstraintKind::EqualCurvature) {
                    -6.4
                } else {
                    -6.0
                },
            ]
        };
        let mut second_bezier_controls = [
            second_bezier_start,
            add_point(&mut document, transform, "bezier 2 middle", second_middle),
            add_point(&mut document, transform, "bezier 2 end", second_end),
        ];
        if matches!(
            subject,
            Family::Constraint {
                kind: geosolve_constraint_editor::ResolvedConstraintKind::EqualCurvature,
                ..
            }
        ) && curvature_option(variant)
            != geosolve_sketch::DocumentCurveCurvatureRelation::MagnitudeOppositeSign
        {
            second_bezier_controls.reverse();
        }
        if endpoint_continuity && variant.option_index >= 4 {
            second_bezier_controls.reverse();
        }
        let beziers = [first_bezier_controls, second_bezier_controls].map(|controls| {
            CurveSpan::line(
                document
                    .add_curve("quadratic", CurveDefinition::QuadraticBezier { controls })
                    .expect("quadratic Bezier"),
            )
        });
        let tangent_shift =
            if displaced(geosolve_constraint_editor::ResolvedConstraintKind::CurveTangency)
                || displaced(geosolve_constraint_editor::ResolvedConstraintKind::CollinearSupports)
            {
                0.2
            } else {
                0.0
            };
        let overlap_start = add_point(&mut document, transform, "overlap a", [-2.0, tangent_shift]);
        let overlap_end = add_point(
            &mut document,
            transform,
            "overlap b",
            [
                2.0 + if displaced(geosolve_constraint_editor::ResolvedConstraintKind::EqualLength)
                {
                    0.5
                } else {
                    0.0
                },
                tangent_shift
                    + if displaced(
                        geosolve_constraint_editor::ResolvedConstraintKind::ParallelLines,
                    ) {
                        0.3
                    } else {
                        0.0
                    },
            ],
        );
        let overlap_reverse = variant.reverse_spans
            ^ (matches!(
                subject,
                Family::Constraint {
                    kind: geosolve_constraint_editor::ResolvedConstraintKind::CurveTangency,
                    ..
                }
            ) && tangent_option(variant) == geosolve_sketch::TangentOrientation::Opposed);
        let overlapping_line = add_line_from_points(
            &mut document,
            "overlapping line",
            overlap_start,
            overlap_end,
            overlap_reverse,
        );
        let radial_base = [intersection_x - 3.0, 3.0];
        let radial_normal_offset =
            if displaced(geosolve_constraint_editor::ResolvedConstraintKind::RadialLine) {
                0.25
            } else {
                0.0
            };
        let radial_start = add_point(
            &mut document,
            transform,
            "radial segment start",
            [radial_base[0] + 2.0, radial_base[1] + radial_normal_offset],
        );
        let radial_end = add_point(
            &mut document,
            transform,
            "radial segment end",
            [radial_base[0] + 3.0, radial_base[1] + radial_normal_offset],
        );
        let radial_line = add_line_from_points(
            &mut document,
            "external radial segment",
            radial_start,
            radial_end,
            variant.reverse_spans,
        );
        let horizontal_contact_parameter = if variant.reverse_spans { 1.0 - p } else { p };
        let overlap_contact_parameter = if overlap_reverse { 1.0 - p } else { p };
        let [radial_start, radial_end] = line_points(&document, radial_line);
        let radial_center = document.point(points[4]).expect("radial center").position;
        let radial_direction = [
            radial_end[0] - radial_start[0],
            radial_end[1] - radial_start[1],
        ];
        let radial_offset = [
            radial_center[0] - radial_start[0],
            radial_center[1] - radial_start[1],
        ];
        let radial_length = radial_direction[0].hypot(radial_direction[1]);
        let radial_unit = [
            radial_direction[0] / radial_length,
            radial_direction[1] / radial_length,
        ];
        let radial_parameter = radial_offset[0]
            .mul_add(radial_unit[0], radial_offset[1] * radial_unit[1])
            / radial_length;
        let original_points = document
            .points()
            .iter()
            .map(|point| (point.id, point.position))
            .collect();
        let original_scalars = document
            .scalars()
            .iter()
            .map(|scalar| (scalar.id, scalar.value))
            .collect();
        let datum_subject = matches!(
            subject,
            Family::Constraint {
                kind: geosolve_constraint_editor::ResolvedConstraintKind::CoincidentWithOrigin
                    | geosolve_constraint_editor::ResolvedConstraintKind::PointOnDatumAxis
                    | geosolve_constraint_editor::ResolvedConstraintKind::CollinearWithDatumAxis
                    | geosolve_constraint_editor::ResolvedConstraintKind::SymmetricAboutDatumAxis,
                ..
            }
        );
        Self {
            document,
            transform,
            points,
            coincident,
            contact_point,
            line_midpoint,
            lines: [first_line, second_line],
            circles: [first_circle, second_circle],
            circle_radii: [first_radius, second_radius],
            beziers,
            overlapping_line,
            radial_line,
            horizontal_contact_parameter,
            overlap_contact_parameter,
            radial_parameter,
            original_points,
            original_scalars,
            pre_satisfied: !datum_subject
                && (displaced_kind.is_none()
                    || displaced_kind
                        == Some(geosolve_constraint_editor::ResolvedConstraintKind::FixedPoint)),
        }
    }

    /// The underlying document (used by the survey driver + validators).
    pub fn document(&self) -> &SketchDocument {
        &self.document
    }

    /// The horizontal contact parameter for the primary line span.
    pub fn horizontal_contact_parameter(&self) -> f64 {
        self.horizontal_contact_parameter
    }

    /// The contact parameter for the overlapping-line span.
    pub fn overlap_contact_parameter(&self) -> f64 {
        self.overlap_contact_parameter
    }

    /// The radial support-line parameter.
    pub fn radial_parameter(&self) -> f64 {
        self.radial_parameter
    }

    /// The radial support-line span.
    pub fn radial_line(&self) -> CurveSpan {
        self.radial_line
    }

    /// The overlapping support-line span.
    pub fn overlapping_line(&self) -> CurveSpan {
        self.overlapping_line
    }

    /// Whether the fixture was built pre-satisfied for this family.
    pub fn pre_satisfied(&self) -> bool {
        self.pre_satisfied
    }

    /// The original (pre-solve) point handles and positions.
    pub fn original_points(&self) -> &[(DesignPointId, [f64; 2])] {
        &self.original_points
    }

    /// The original (pre-solve) scalar handles and values.
    pub fn original_scalars(&self) -> &[(DesignScalarId, f64)] {
        &self.original_scalars
    }

    /// The primary and cross line spans.
    pub fn lines(&self) -> [CurveSpan; 2] {
        self.lines
    }

    /// The two circle spans.
    pub fn circles(&self) -> [CurveSpan; 2] {
        self.circles
    }

    /// The two quadratic Bezier spans.
    pub fn beziers(&self) -> [CurveSpan; 2] {
        self.beziers
    }

    /// The two coincident point handles.
    pub fn coincident(&self) -> [DesignPointId; 2] {
        self.coincident
    }

    /// The contact-point handle.
    pub fn contact_point(&self) -> DesignPointId {
        self.contact_point
    }

    /// The line-midpoint handle.
    pub fn line_midpoint(&self) -> DesignPointId {
        self.line_midpoint
    }

    /// The six primary point handles.
    pub fn points(&self) -> [DesignPointId; 6] {
        self.points
    }
}

fn add_point(
    document: &mut SketchDocument,
    transform: Transform,
    label: &str,
    position: [f64; 2],
) -> DesignPointId {
    document
        .add_point(label, transform.point(position))
        .unwrap_or_else(|error| panic!("add oracle point {label}: {error}"))
}

fn add_line_from_points(
    document: &mut SketchDocument,
    label: &str,
    first: DesignPointId,
    second: DesignPointId,
    reverse: bool,
) -> CurveSpan {
    let [start, end] = if reverse {
        [second, first]
    } else {
        [first, second]
    };
    let start_position = document.point(start).expect("line start point").position;
    let end_position = document.point(end).expect("line end point").position;
    let delta = [
        end_position[0] - start_position[0],
        end_position[1] - start_position[1],
    ];
    let length = delta[0].hypot(delta[1]);
    // A zero-length line has an undefined direction: `delta / length` would be
    // NaN, which the document rejects. Fall back to an axis-aligned unit
    // direction so a degenerate line is still a valid, finite document field.
    let branch_direction = if length == 0.0 {
        [1.0, 0.0]
    } else {
        [delta[0] / length, delta[1] / length]
    };
    CurveSpan::line(
        document
            .add_curve(
                label,
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction,
                },
            )
            .unwrap_or_else(|error| panic!("add oracle line {label}: {error}")),
    )
}

pub fn continuity_option(variant: FuzzVariant) -> geosolve_sketch::DocumentCurveContinuity {
    match variant.option_index % 4 {
        0 => geosolve_sketch::DocumentCurveContinuity::G1,
        1 => geosolve_sketch::DocumentCurveContinuity::G0,
        2 => geosolve_sketch::DocumentCurveContinuity::G2,
        3 => geosolve_sketch::DocumentCurveContinuity::ParametricC2 {
            first_rate: if variant.swap_operands { 2.0 } else { 1.0 },
            second_rate: if variant.swap_operands { 1.0 } else { 2.0 },
        },
        _ => unreachable!("modulo four"),
    }
}

pub fn curvature_option(variant: FuzzVariant) -> geosolve_sketch::DocumentCurveCurvatureRelation {
    match variant.option_index % 3 {
        0 => geosolve_sketch::DocumentCurveCurvatureRelation::MagnitudeOppositeSign,
        1 => geosolve_sketch::DocumentCurveCurvatureRelation::Signed,
        2 => geosolve_sketch::DocumentCurveCurvatureRelation::MagnitudeSameSign,
        _ => unreachable!("modulo three"),
    }
}

pub fn tangent_option(variant: FuzzVariant) -> geosolve_sketch::TangentOrientation {
    if variant.option_index.is_multiple_of(2) {
        geosolve_sketch::TangentOrientation::Aligned
    } else {
        geosolve_sketch::TangentOrientation::Opposed
    }
}

fn line_points(document: &SketchDocument, span: CurveSpan) -> [[f64; 2]; 2] {
    let curve = document
        .curve(span.curve)
        .expect("radial fixture line curve");
    let CurveDefinition::Line { start, end, .. } = curve.definition else {
        panic!("radial fixture line is not a line: {:?}", curve.definition);
    };
    [
        document.point(start).expect("radial start").position,
        document.point(end).expect("radial end").position,
    ]
}

/// Build the constraint operands for a family + variant.
pub fn constraint_operands(
    kind: geosolve_constraint_editor::ResolvedConstraintKind,
    fixture: &MatrixFixture,
    variant: FuzzVariant,
) -> Vec<AuthoringOperand> {
    let selected = |item: SelectionItem| AuthoringOperand::selected(item);
    let picked =
        |item: SelectionItem, parameter: f64| AuthoringOperand::picked(item, Some(parameter));
    let p = fixture.horizontal_contact_parameter();
    let mut operands = match kind {
        geosolve_constraint_editor::ResolvedConstraintKind::FixedPoint => {
            vec![selected(SelectionItem::Point(fixture.points[0]))]
        }
        geosolve_constraint_editor::ResolvedConstraintKind::CoincidentWithOrigin => vec![
            selected(SelectionItem::Point(fixture.contact_point)),
            selected(SelectionItem::Datum(SketchDatum::Origin)),
        ],
        geosolve_constraint_editor::ResolvedConstraintKind::PointOnDatumAxis => vec![
            selected(SelectionItem::Point(fixture.contact_point)),
            selected(SelectionItem::Datum(oracle_axis_datum(variant).0)),
        ],
        geosolve_constraint_editor::ResolvedConstraintKind::CoincidentPoints => fixture
            .coincident
            .map(SelectionItem::Point)
            .map(selected)
            .to_vec(),
        geosolve_constraint_editor::ResolvedConstraintKind::PointOnCurve => vec![
            selected(SelectionItem::Point(fixture.contact_point)),
            picked(SelectionItem::Curve(fixture.lines[0]), p),
        ],
        geosolve_constraint_editor::ResolvedConstraintKind::CurveContact => vec![
            picked(SelectionItem::Curve(fixture.lines[0]), p),
            picked(SelectionItem::Curve(fixture.lines[1]), 0.5),
        ],
        geosolve_constraint_editor::ResolvedConstraintKind::HorizontalLine => {
            vec![selected(SelectionItem::Curve(fixture.lines[0]))]
        }
        geosolve_constraint_editor::ResolvedConstraintKind::VerticalLine => {
            vec![selected(SelectionItem::Curve(fixture.lines[1]))]
        }
        geosolve_constraint_editor::ResolvedConstraintKind::HorizontalPoints => fixture.points
            [0..2]
            .iter()
            .copied()
            .map(SelectionItem::Point)
            .map(selected)
            .collect(),
        geosolve_constraint_editor::ResolvedConstraintKind::VerticalPoints => fixture.points[2..4]
            .iter()
            .copied()
            .map(SelectionItem::Point)
            .map(selected)
            .collect(),
        geosolve_constraint_editor::ResolvedConstraintKind::ConcentricCurves
        | geosolve_constraint_editor::ResolvedConstraintKind::EqualRadius => fixture
            .circles
            .map(SelectionItem::Curve)
            .map(selected)
            .to_vec(),
        geosolve_constraint_editor::ResolvedConstraintKind::CollinearSupports
        | geosolve_constraint_editor::ResolvedConstraintKind::ParallelLines
        | geosolve_constraint_editor::ResolvedConstraintKind::EqualLength => vec![
            selected(SelectionItem::Curve(fixture.lines[0])),
            selected(SelectionItem::Curve(fixture.overlapping_line)),
        ],
        geosolve_constraint_editor::ResolvedConstraintKind::CollinearWithDatumAxis => vec![
            selected(SelectionItem::Curve(fixture.lines[0])),
            selected(SelectionItem::Datum(oracle_axis_datum(variant).0)),
        ],
        geosolve_constraint_editor::ResolvedConstraintKind::PerpendicularLines => fixture
            .lines
            .map(SelectionItem::Curve)
            .map(selected)
            .to_vec(),
        geosolve_constraint_editor::ResolvedConstraintKind::RadialLine => vec![
            picked(SelectionItem::Curve(fixture.circles[0]), 0.0),
            picked(
                SelectionItem::Curve(fixture.radial_line()),
                variant.contact_parameter,
            ),
        ],
        geosolve_constraint_editor::ResolvedConstraintKind::EqualCurvature => fixture
            .beziers
            .map(|span| picked(SelectionItem::Curve(span), 0.5))
            .to_vec(),
        geosolve_constraint_editor::ResolvedConstraintKind::Midpoint => vec![
            selected(SelectionItem::Point(fixture.line_midpoint)),
            selected(SelectionItem::Curve(fixture.lines[0])),
        ],
        geosolve_constraint_editor::ResolvedConstraintKind::SymmetricAboutLine => vec![
            selected(SelectionItem::Point(fixture.points[4])),
            selected(SelectionItem::Point(fixture.points[5])),
            selected(SelectionItem::Curve(fixture.lines[1])),
        ],
        geosolve_constraint_editor::ResolvedConstraintKind::SymmetricAboutDatumAxis => vec![
            selected(SelectionItem::Point(fixture.points[4])),
            selected(SelectionItem::Point(fixture.points[5])),
            selected(SelectionItem::Datum(oracle_axis_datum(variant).0)),
        ],
        geosolve_constraint_editor::ResolvedConstraintKind::CurveTangency => vec![
            picked(SelectionItem::Curve(fixture.lines[0]), p),
            picked(
                SelectionItem::Curve(fixture.overlapping_line),
                fixture.overlap_contact_parameter(),
            ),
        ],
        geosolve_constraint_editor::ResolvedConstraintKind::EndpointContinuity => vec![
            picked(SelectionItem::Curve(fixture.beziers[0]), 1.0),
            picked(
                SelectionItem::Curve(fixture.beziers[1]),
                if variant.option_index >= 4 { 1.0 } else { 0.0 },
            ),
        ],
    };
    if variant.swap_operands {
        match kind {
            geosolve_constraint_editor::ResolvedConstraintKind::CoincidentPoints
            | geosolve_constraint_editor::ResolvedConstraintKind::CoincidentWithOrigin
            | geosolve_constraint_editor::ResolvedConstraintKind::PointOnDatumAxis
            | geosolve_constraint_editor::ResolvedConstraintKind::PointOnCurve
            | geosolve_constraint_editor::ResolvedConstraintKind::CurveContact
            | geosolve_constraint_editor::ResolvedConstraintKind::HorizontalPoints
            | geosolve_constraint_editor::ResolvedConstraintKind::VerticalPoints
            | geosolve_constraint_editor::ResolvedConstraintKind::ConcentricCurves
            | geosolve_constraint_editor::ResolvedConstraintKind::CollinearSupports
            | geosolve_constraint_editor::ResolvedConstraintKind::CollinearWithDatumAxis
            | geosolve_constraint_editor::ResolvedConstraintKind::ParallelLines
            | geosolve_constraint_editor::ResolvedConstraintKind::PerpendicularLines
            | geosolve_constraint_editor::ResolvedConstraintKind::RadialLine
            | geosolve_constraint_editor::ResolvedConstraintKind::EqualLength
            | geosolve_constraint_editor::ResolvedConstraintKind::EqualRadius
            | geosolve_constraint_editor::ResolvedConstraintKind::EqualCurvature
            | geosolve_constraint_editor::ResolvedConstraintKind::Midpoint
            | geosolve_constraint_editor::ResolvedConstraintKind::CurveTangency
            | geosolve_constraint_editor::ResolvedConstraintKind::EndpointContinuity
            | geosolve_constraint_editor::ResolvedConstraintKind::SymmetricAboutDatumAxis => {
                operands.reverse();
            }
            geosolve_constraint_editor::ResolvedConstraintKind::SymmetricAboutLine => {
                operands.swap(0, 1);
            }
            geosolve_constraint_editor::ResolvedConstraintKind::FixedPoint
            | geosolve_constraint_editor::ResolvedConstraintKind::HorizontalLine
            | geosolve_constraint_editor::ResolvedConstraintKind::VerticalLine => {}
        }
    }
    operands
}

/// Build the dimension operands for a family + variant.
pub fn dimension_operands(
    kind: geosolve_constraint_editor::DimensionKind,
    fixture: &MatrixFixture,
    variant: FuzzVariant,
) -> Vec<AuthoringOperand> {
    let selected = |item: SelectionItem| AuthoringOperand::selected(item);
    let mut operands = match kind {
        geosolve_constraint_editor::DimensionKind::PointDistance => fixture.points[0..2]
            .iter()
            .copied()
            .map(SelectionItem::Point)
            .map(selected)
            .collect(),
        geosolve_constraint_editor::DimensionKind::SegmentLength => {
            vec![selected(SelectionItem::Curve(fixture.lines[0]))]
        }
        geosolve_constraint_editor::DimensionKind::Radius => {
            vec![selected(SelectionItem::Curve(fixture.circles[0]))]
        }
        geosolve_constraint_editor::DimensionKind::Diameter => {
            vec![selected(SelectionItem::Curve(fixture.circles[1]))]
        }
        geosolve_constraint_editor::DimensionKind::OrientedAngle => fixture
            .lines
            .map(SelectionItem::Curve)
            .map(selected)
            .to_vec(),
    };
    if variant.swap_operands
        && matches!(
            kind,
            geosolve_constraint_editor::DimensionKind::PointDistance
                | geosolve_constraint_editor::DimensionKind::OrientedAngle
        )
    {
        operands.reverse();
    }
    operands
}

const fn oracle_axis_datum(variant: FuzzVariant) -> (SketchDatum, DocumentCoordinateAxis) {
    if variant.option_index.is_multiple_of(2) {
        (SketchDatum::XAxis, DocumentCoordinateAxis::X)
    } else {
        (SketchDatum::YAxis, DocumentCoordinateAxis::Y)
    }
}
