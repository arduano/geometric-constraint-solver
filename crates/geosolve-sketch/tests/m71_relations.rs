// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::{AuditEvaluationStatus, OperationOutcome, ResidualCategory, SolverConfig};
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DesignPointId, DocumentArcSweep, DocumentCenterRef,
    DocumentCommand, DocumentCommandEffect, DocumentConstraintDefinition, DocumentConstraintId,
    DocumentDirectionSense, DocumentEdit, DocumentError, DocumentHyperbolaBranch,
    DocumentLineSupportRef, DocumentObjectId, DocumentSessionError, DocumentSolveRequest,
    DocumentSourceId, PreparedSketchOperation, PreparedSketchPatch, RetainedSketchDocumentSession,
    RuntimeSource, ScalarDomain, ScalarUnit, SketchDocument, SketchDocumentSession,
    SketchSolveRequest, SketchSource,
};

const HARD_TOLERANCE: f64 = 1.0e-9;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RelationKind {
    HorizontalPoints,
    VerticalPoints,
    Concentric,
    Collinear,
}

impl RelationKind {
    const ALL: [Self; 4] = [
        Self::HorizontalPoints,
        Self::VerticalPoints,
        Self::Concentric,
        Self::Collinear,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::HorizontalPoints => "horizontal points",
            Self::VerticalPoints => "vertical points",
            Self::Concentric => "concentric",
            Self::Collinear => "collinear",
        }
    }

    const fn expected_rows(self) -> usize {
        match self {
            Self::HorizontalPoints | Self::VerticalPoints => 1,
            Self::Concentric | Self::Collinear => 2,
        }
    }

    const fn expected_rank_gain(self) -> usize {
        self.expected_rows()
    }
}

#[derive(Clone, Copy)]
struct Transform {
    scale: f64,
    angle: f64,
    translation: [f64; 2],
}

impl Transform {
    fn point(self, point: [f64; 2]) -> [f64; 2] {
        let (sin, cos) = self.angle.sin_cos();
        [
            self.scale * (cos * point[0] - sin * point[1] + self.translation[0]),
            self.scale * (sin * point[0] + cos * point[1] + self.translation[1]),
        ]
    }
}

#[derive(Clone)]
struct RelationFixture {
    kind: RelationKind,
    document: SketchDocument,
    points: Vec<DesignPointId>,
    curves: Vec<geosolve_sketch::CurveId>,
}

impl RelationFixture {
    fn definition(
        &self,
        swap_operands: bool,
        first_direction: DocumentDirectionSense,
        second_direction: DocumentDirectionSense,
    ) -> DocumentConstraintDefinition {
        match self.kind {
            RelationKind::HorizontalPoints => {
                let (first, second) = if swap_operands {
                    (self.points[1], self.points[0])
                } else {
                    (self.points[0], self.points[1])
                };
                DocumentConstraintDefinition::HorizontalPoints { first, second }
            }
            RelationKind::VerticalPoints => {
                let (first, second) = if swap_operands {
                    (self.points[1], self.points[0])
                } else {
                    (self.points[0], self.points[1])
                };
                DocumentConstraintDefinition::VerticalPoints { first, second }
            }
            RelationKind::Concentric => {
                let (first, second) = if swap_operands {
                    (self.curves[1], self.curves[0])
                } else {
                    (self.curves[0], self.curves[1])
                };
                DocumentConstraintDefinition::Concentric {
                    first: DocumentCenterRef { curve: first },
                    second: DocumentCenterRef { curve: second },
                }
            }
            RelationKind::Collinear => {
                let first = DocumentLineSupportRef {
                    span: CurveSpan::line(self.curves[0]),
                    direction: first_direction,
                };
                let second = DocumentLineSupportRef {
                    span: CurveSpan::line(self.curves[1]),
                    direction: second_direction,
                };
                let (first, second) = if swap_operands {
                    (second, first)
                } else {
                    (first, second)
                };
                DocumentConstraintDefinition::Collinear { first, second }
            }
        }
    }

    fn default_definition(&self) -> DocumentConstraintDefinition {
        self.definition(
            false,
            DocumentDirectionSense::Forward,
            DocumentDirectionSense::Forward,
        )
    }

    fn first_dependency(&self) -> DocumentObjectId {
        match self.kind {
            RelationKind::HorizontalPoints | RelationKind::VerticalPoints => {
                DocumentObjectId::Point(self.points[0])
            }
            RelationKind::Concentric | RelationKind::Collinear => {
                DocumentObjectId::Curve(self.curves[0])
            }
        }
    }

    fn add_relation(&mut self) -> DocumentConstraintId {
        self.document
            .add_constraint(self.kind.label(), self.default_definition())
            .unwrap()
    }

    fn fix_all_points_at_design_positions(&mut self) {
        for (index, point) in self.points.clone().into_iter().enumerate() {
            let target = self.document.point(point).unwrap().position;
            self.document
                .add_constraint(
                    format!("fixed conflict witness {index}"),
                    DocumentConstraintDefinition::FixedPoint { point, target },
                )
                .unwrap();
        }
    }
}

fn line(
    document: &mut SketchDocument,
    label: &str,
    start: DesignPointId,
    end: DesignPointId,
) -> geosolve_sketch::CurveId {
    let start_position = document.point(start).unwrap().position;
    let end_position = document.point(end).unwrap().position;
    let delta = [
        end_position[0] - start_position[0],
        end_position[1] - start_position[1],
    ];
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

fn circle(
    document: &mut SketchDocument,
    label: &str,
    center: DesignPointId,
    radius: f64,
) -> geosolve_sketch::CurveId {
    let radius = document
        .add_scalar(
            format!("{label} radius"),
            radius,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    document
        .add_curve(label, CurveDefinition::Circle { center, radius })
        .unwrap()
}

#[derive(Clone, Copy, Debug)]
enum StoredCenterFamily {
    Circle,
    CircularArc,
    Ellipse,
    EllipticalArc,
    HyperbolaSegment,
}

impl StoredCenterFamily {
    const ALL: [Self; 5] = [
        Self::Circle,
        Self::CircularArc,
        Self::Ellipse,
        Self::EllipticalArc,
        Self::HyperbolaSegment,
    ];
}

fn finite_scalar(
    document: &mut SketchDocument,
    label: &str,
    value: f64,
    unit: ScalarUnit,
    domain: ScalarDomain,
) -> geosolve_sketch::DesignScalarId {
    document.add_scalar(label, value, unit, domain).unwrap()
}

fn centered_curve(
    document: &mut SketchDocument,
    family: StoredCenterFamily,
    label: &str,
    center: DesignPointId,
) -> geosolve_sketch::CurveId {
    let center_position = document.point(center).unwrap().position;
    let radius = finite_scalar(
        document,
        &format!("{label} radius"),
        1.25,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    );
    let axis = document
        .add_point(
            format!("{label} axis"),
            [center_position[0] + 2.0, center_position[1] + 0.25],
        )
        .unwrap();
    let ratio = finite_scalar(
        document,
        &format!("{label} ratio"),
        0.6,
        ScalarUnit::Parameter,
        ScalarDomain::Bounded {
            lower: f64::from_bits(1),
            upper: 1.0,
        },
    );
    let arc_start = finite_scalar(
        document,
        &format!("{label} arc start"),
        -0.4,
        ScalarUnit::Angle,
        ScalarDomain::Finite,
    );
    let arc_end = finite_scalar(
        document,
        &format!("{label} arc end"),
        1.2,
        ScalarUnit::Angle,
        ScalarDomain::Finite,
    );
    let trim_start = finite_scalar(
        document,
        &format!("{label} trim start"),
        -0.4,
        ScalarUnit::Parameter,
        ScalarDomain::Finite,
    );
    let trim_end = finite_scalar(
        document,
        &format!("{label} trim end"),
        1.2,
        ScalarUnit::Parameter,
        ScalarDomain::Finite,
    );
    let definition = match family {
        StoredCenterFamily::Circle => CurveDefinition::Circle { center, radius },
        StoredCenterFamily::CircularArc => CurveDefinition::CircularArc {
            center,
            radius,
            start_angle: arc_start,
            end_angle: arc_end,
            sweep: DocumentArcSweep::CounterClockwise,
        },
        StoredCenterFamily::Ellipse => CurveDefinition::Ellipse {
            center,
            major_axis_point: axis,
            minor_axis_ratio: ratio,
        },
        StoredCenterFamily::EllipticalArc => CurveDefinition::EllipticalArc {
            center,
            major_axis_point: axis,
            minor_axis_ratio: ratio,
            start_angle: arc_start,
            end_angle: arc_end,
            sweep: DocumentArcSweep::CounterClockwise,
        },
        StoredCenterFamily::HyperbolaSegment => CurveDefinition::HyperbolaSegment {
            center,
            transverse_axis_point: axis,
            semi_conjugate: radius,
            branch: DocumentHyperbolaBranch::Positive,
            trim_start,
            trim_end,
        },
    };
    document.add_curve(label, definition).unwrap()
}

fn fixture(kind: RelationKind, transform: Transform) -> RelationFixture {
    let mut document = SketchDocument::new(transform.scale).unwrap();
    let (points, curves) = match kind {
        RelationKind::HorizontalPoints | RelationKind::VerticalPoints => {
            let points = [[-1.1, -0.35], [1.4, 0.65]]
                .map(|position| {
                    document
                        .add_point("point", transform.point(position))
                        .unwrap()
                })
                .to_vec();
            (points, Vec::new())
        }
        RelationKind::Concentric => {
            let points = [[-0.4, -0.25], [0.65, 0.55]]
                .map(|position| {
                    document
                        .add_point("center", transform.point(position))
                        .unwrap()
                })
                .to_vec();
            let curves = vec![
                circle(&mut document, "first circle", points[0], transform.scale),
                circle(
                    &mut document,
                    "second circle",
                    points[1],
                    1.7 * transform.scale,
                ),
            ];
            (points, curves)
        }
        RelationKind::Collinear => {
            let points = [[-1.4, -0.45], [0.8, 0.25], [-0.7, 1.15], [1.35, 1.55]]
                .map(|position| {
                    document
                        .add_point("line point", transform.point(position))
                        .unwrap()
                })
                .to_vec();
            let curves = vec![
                line(&mut document, "first line", points[0], points[1]),
                line(&mut document, "second line", points[2], points[3]),
            ];
            (points, curves)
        }
    };
    RelationFixture {
        kind,
        document,
        points,
        curves,
    }
}

fn default_transform() -> Transform {
    Transform {
        scale: 1.0,
        angle: 0.0,
        translation: [0.0, 0.0],
    }
}

fn source_id(document: &SketchDocument, constraint: DocumentConstraintId) -> DocumentSourceId {
    document.constraint(constraint).unwrap().source_id
}

fn runtime_row_count(document: &SketchDocument, constraint: DocumentConstraintId) -> Option<usize> {
    let source = source_id(document, constraint);
    let lowered = document.lower().unwrap();
    let RuntimeSource::Constraint(runtime) = lowered.mappings().runtime_source(source)? else {
        panic!("M71 relation must lower to a runtime constraint")
    };
    let compiled = lowered
        .sketch()
        .compile(SketchSolveRequest::default().without_previous_state_preferences())
        .unwrap();
    let mapping = compiled
        .source_mappings()
        .iter()
        .find(|mapping| mapping.source == SketchSource::Constraint(runtime))
        .unwrap();
    Some(
        mapping
            .residual_ids
            .iter()
            .map(|residual| {
                compiled
                    .problem()
                    .residual(*residual)
                    .unwrap()
                    .output_dimension()
            })
            .sum(),
    )
}

fn accepted_session(document: SketchDocument) -> RetainedSketchDocumentSession {
    RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default().without_previous_state_preferences(),
        SolverConfig::default(),
    )
    .unwrap()
}

fn assert_finite_accepted_relation(
    session: &RetainedSketchDocumentSession,
    constraint: DocumentConstraintId,
    expected_rows: usize,
) {
    let accepted = session.accepted_state().expect("accepted M71 relation");
    let solve = accepted.solve_result();
    assert!(solve.accepted());
    assert!(
        solve
            .acceptance_hard_residual_max
            .is_some_and(|maximum| maximum.is_finite() && maximum <= HARD_TOLERANCE)
    );
    for point in &solve.geometry.points {
        assert!(point.position.x.is_finite() && point.position.y.is_finite());
    }
    for circle in &solve.geometry.circles {
        assert!(
            circle.center.x.is_finite()
                && circle.center.y.is_finite()
                && circle.radius.is_finite()
                && circle.radius > 0.0
        );
    }
    for arc in &solve.geometry.arcs {
        assert!(
            arc.center.x.is_finite()
                && arc.center.y.is_finite()
                && arc.radius.is_finite()
                && arc.radius > 0.0
                && arc.start_angle.is_finite()
                && arc.end_angle.is_finite()
                && arc.signed_sweep.is_finite()
        );
    }
    for conic in &solve.geometry.conics {
        conic.geometry().expect("accepted conic geometry is finite");
    }
    assert!(
        solve
            .geometry
            .nurbs
            .iter()
            .flat_map(|nurbs| &nurbs.weights)
            .all(|weight| weight.is_finite() && *weight > 0.0)
    );

    let source = source_id(accepted.document(), constraint);
    let RuntimeSource::Constraint(runtime) = accepted.mappings().runtime_source(source).unwrap()
    else {
        panic!("M71 relation must retain a runtime constraint mapping")
    };
    let core_source = solve
        .source_mappings
        .iter()
        .find(|mapping| mapping.source == SketchSource::Constraint(runtime))
        .and_then(|mapping| mapping.core_source_id)
        .unwrap();
    let audit = solve
        .display_audit
        .sources
        .iter()
        .find(|audit| audit.source_id == core_source)
        .unwrap();
    assert_eq!(audit.rows.len(), expected_rows);
    assert!(audit.rows.iter().all(|row| {
        row.category == ResidualCategory::Hard
            && row.evaluation_status == AuditEvaluationStatus::Evaluated
            && row.raw_residual.is_finite()
            && row.normalized_residual.is_finite()
            && row.normalized_residual.abs() <= HARD_TOLERANCE
    }));

    let diagnostics = accepted.diagnostics();
    let diagnostic = diagnostics
        .sources
        .iter()
        .find(|diagnostic| diagnostic.source == source)
        .unwrap();
    assert_eq!(diagnostic.active_row_count, expected_rows);
    assert_eq!(diagnostic.evaluated_row_count, expected_rows);
    assert_eq!(diagnostic.failed_row_count, 0);
    assert!(
        diagnostic
            .maximum_normalized_residual
            .is_some_and(|maximum| maximum.is_finite() && maximum <= HARD_TOLERANCE)
    );
}

fn accepted_positions(
    session: &RetainedSketchDocumentSession,
    points: &[DesignPointId],
) -> Vec<[f64; 2]> {
    let document = session.accepted_state().unwrap().document();
    points
        .iter()
        .map(|point| document.point(*point).unwrap().position)
        .collect()
}

fn assert_relation_geometry(kind: RelationKind, positions: &[[f64; 2]], model_scale: f64) {
    let normalized = |value: f64| value.abs() / model_scale;
    match kind {
        RelationKind::HorizontalPoints => {
            assert!(normalized(positions[1][1] - positions[0][1]) <= HARD_TOLERANCE);
        }
        RelationKind::VerticalPoints => {
            assert!(normalized(positions[1][0] - positions[0][0]) <= HARD_TOLERANCE);
        }
        RelationKind::Concentric => {
            assert!(normalized(positions[1][0] - positions[0][0]) <= HARD_TOLERANCE);
            assert!(normalized(positions[1][1] - positions[0][1]) <= HARD_TOLERANCE);
        }
        RelationKind::Collinear => {
            let first = [
                positions[1][0] - positions[0][0],
                positions[1][1] - positions[0][1],
            ];
            let second = [
                positions[3][0] - positions[2][0],
                positions[3][1] - positions[2][1],
            ];
            let first_length = first[0].hypot(first[1]);
            let second_length = second[0].hypot(second[1]);
            assert!(first_length.is_finite() && first_length > 0.0);
            assert!(second_length.is_finite() && second_length > 0.0);
            let direction_cross =
                (first[0] * second[1] - first[1] * second[0]) / (first_length * second_length);
            let offset = [
                positions[2][0] - positions[0][0],
                positions[2][1] - positions[0][1],
            ];
            let support_cross =
                (first[0] * offset[1] - first[1] * offset[0]) / (first_length * model_scale);
            assert!(direction_cross.abs() <= HARD_TOLERANCE);
            assert!(support_cross.abs() <= HARD_TOLERANCE);
        }
    }
}

fn completed_patch(outcome: OperationOutcome<PreparedSketchPatch>) -> PreparedSketchPatch {
    match outcome {
        OperationOutcome::Completed { value, .. } => value,
        OperationOutcome::Cancelled { .. } => panic!("prepared relation job was cancelled"),
        OperationOutcome::WorkExhausted { .. } => {
            panic!("prepared relation job exhausted its work budget")
        }
        _ => panic!("unknown prepared relation outcome"),
    }
}

fn contains_object(document: &SketchDocument, object: DocumentObjectId) -> bool {
    match object {
        DocumentObjectId::Point(point) => document.point(point).is_some(),
        DocumentObjectId::Curve(curve) => document.curve(curve).is_some(),
        DocumentObjectId::Scalar(scalar) => document.scalar(scalar).is_some(),
        DocumentObjectId::Contact(contact) => document.contact(contact).is_some(),
        DocumentObjectId::Constraint(constraint) => document.constraint(constraint).is_some(),
        DocumentObjectId::Dimension(dimension) => document.dimension(dimension).is_some(),
        DocumentObjectId::Parameter(parameter) => document.parameter(parameter).is_some(),
        DocumentObjectId::ExternalBinding(binding) => document.external_binding(binding).is_some(),
    }
}

#[test]
fn m71_relations_have_exact_runtime_and_audit_rows_across_transforms_and_scales() {
    let transforms = [
        Transform {
            scale: 1.0e-6,
            angle: 0.37,
            translation: [3.25, -2.75],
        },
        Transform {
            scale: 1.0,
            angle: -0.81,
            translation: [-4.5, 6.25],
        },
        Transform {
            scale: 1.0e6,
            angle: 1.19,
            translation: [0.125, -0.375],
        },
    ];
    for kind in RelationKind::ALL {
        for transform in transforms {
            let mut fixture = fixture(kind, transform);
            let constraint = fixture.add_relation();
            assert_eq!(
                runtime_row_count(&fixture.document, constraint),
                Some(kind.expected_rows()),
                "{kind:?}, scale={:e}",
                transform.scale
            );
            let session = accepted_session(fixture.document);
            assert_finite_accepted_relation(&session, constraint, kind.expected_rows());
            let positions = accepted_positions(&session, &fixture.points);
            assert_relation_geometry(kind, &positions, transform.scale);
        }
    }
}

#[test]
fn m71_concentric_accepts_every_stored_center_curve_family_in_both_operand_orders() {
    for family in StoredCenterFamily::ALL {
        for swap_operands in [false, true] {
            let mut document = SketchDocument::new(1.0).unwrap();
            let first_center = document.add_point("family center", [-2.0, 1.0]).unwrap();
            let second_center = document.add_point("circle center", [2.0, -1.0]).unwrap();
            let family_curve =
                centered_curve(&mut document, family, "stored-center family", first_center);
            let circle_curve = circle(&mut document, "reference circle", second_center, 0.8);
            assert_eq!(
                document
                    .resolve_center_ref(DocumentCenterRef {
                        curve: family_curve,
                    })
                    .unwrap(),
                first_center,
                "{family:?}"
            );
            let (first, second) = if swap_operands {
                (circle_curve, family_curve)
            } else {
                (family_curve, circle_curve)
            };
            let constraint = document
                .add_constraint(
                    "stored-center family concentric",
                    DocumentConstraintDefinition::Concentric {
                        first: DocumentCenterRef { curve: first },
                        second: DocumentCenterRef { curve: second },
                    },
                )
                .unwrap();
            assert_eq!(runtime_row_count(&document, constraint), Some(2));

            let session = accepted_session(document);
            assert_finite_accepted_relation(&session, constraint, 2);
            let positions = accepted_positions(&session, &[first_center, second_center]);
            assert!(
                (positions[1][0] - positions[0][0]).abs() <= HARD_TOLERANCE,
                "{family:?}, swap={swap_operands}"
            );
            assert!(
                (positions[1][1] - positions[0][1]).abs() <= HARD_TOLERANCE,
                "{family:?}, swap={swap_operands}"
            );
        }
    }
}

#[test]
fn m71_relations_reduce_rank_and_dof_by_their_independent_row_count() {
    for kind in RelationKind::ALL {
        let base = fixture(kind, default_transform());
        let baseline = accepted_session(base.document.clone());
        let baseline_rank = baseline
            .accepted_state()
            .unwrap()
            .diagnostics()
            .rank
            .unwrap();
        assert!(baseline_rank.numerical_valid);

        let mut constrained = base;
        let constraint = constrained.add_relation();
        let constrained = accepted_session(constrained.document);
        assert_finite_accepted_relation(&constrained, constraint, kind.expected_rows());
        let constrained_rank = constrained
            .accepted_state()
            .unwrap()
            .diagnostics()
            .rank
            .unwrap();
        assert!(constrained_rank.numerical_valid);
        assert_eq!(
            constrained_rank.numerical_rank.unwrap(),
            baseline_rank.numerical_rank.unwrap() + kind.expected_rank_gain(),
            "{kind:?} numerical rank"
        );
        assert_eq!(
            constrained_rank.numerical_right_nullity.unwrap() + kind.expected_rank_gain(),
            baseline_rank.numerical_right_nullity.unwrap(),
            "{kind:?} equality DOF"
        );
    }
}

#[test]
fn m71_operand_order_is_commutative_and_collinear_support_direction_is_explicit_but_equivalent() {
    for kind in RelationKind::ALL {
        let direction_cases: &[(DocumentDirectionSense, DocumentDirectionSense)] =
            if kind == RelationKind::Collinear {
                &[
                    (
                        DocumentDirectionSense::Forward,
                        DocumentDirectionSense::Forward,
                    ),
                    (
                        DocumentDirectionSense::Forward,
                        DocumentDirectionSense::Reverse,
                    ),
                    (
                        DocumentDirectionSense::Reverse,
                        DocumentDirectionSense::Forward,
                    ),
                    (
                        DocumentDirectionSense::Reverse,
                        DocumentDirectionSense::Reverse,
                    ),
                ]
            } else {
                &[(
                    DocumentDirectionSense::Forward,
                    DocumentDirectionSense::Forward,
                )]
            };
        for swap in [false, true] {
            for &(first_direction, second_direction) in direction_cases {
                let mut fixture = fixture(
                    kind,
                    Transform {
                        scale: 7.0,
                        angle: 0.43,
                        translation: [2.0, -3.0],
                    },
                );
                let definition = fixture.definition(swap, first_direction, second_direction);
                let constraint = fixture
                    .document
                    .add_constraint("commutative relation", definition)
                    .unwrap();
                let session = accepted_session(fixture.document);
                assert_finite_accepted_relation(&session, constraint, kind.expected_rows());
                if kind == RelationKind::Collinear {
                    let accepted = session.accepted_state().unwrap();
                    let source = source_id(accepted.document(), constraint);
                    let RuntimeSource::Constraint(runtime) =
                        accepted.mappings().runtime_source(source).unwrap()
                    else {
                        panic!("Collinear must retain a runtime constraint mapping")
                    };
                    let core_source = accepted
                        .solve_result()
                        .source_mappings
                        .iter()
                        .find(|mapping| mapping.source == SketchSource::Constraint(runtime))
                        .and_then(|mapping| mapping.core_source_id)
                        .unwrap();
                    let audit = accepted
                        .solve_result()
                        .display_audit
                        .sources
                        .iter()
                        .find(|audit| audit.source_id == core_source)
                        .unwrap();
                    let expected = if swap {
                        [("first", "second line"), ("second", "first line")]
                    } else {
                        [("first", "first line"), ("second", "second line")]
                    };
                    for row in &audit.rows {
                        for (name, value) in expected {
                            assert!(
                                row.bindings.iter().any(|binding| {
                                    binding.name == name && binding.value == value
                                }),
                                "missing persistent {name}={value:?} binding for swap={swap}, \
                                 directions={first_direction:?}/{second_direction:?}: {:?}",
                                row.bindings
                            );
                        }
                    }
                }
                let positions = accepted_positions(&session, &fixture.points);
                assert_relation_geometry(kind, &positions, 7.0);
                let diagnostics = session.accepted_state().unwrap().diagnostics();
                let rank = diagnostics.rank.unwrap();
                assert_eq!(
                    rank.numerical_rank,
                    Some(kind.expected_rank_gain()),
                    "{kind:?}, swap={swap}, directions={first_direction:?}/{second_direction:?}"
                );
            }
        }
    }
}

#[test]
fn m71_duplicate_relations_are_accepted_and_diagnosed_as_redundant() {
    for kind in RelationKind::ALL {
        let mut fixture = fixture(kind, default_transform());
        let first = fixture.add_relation();
        let second = fixture
            .document
            .add_constraint("duplicate relation", fixture.default_definition())
            .unwrap();
        let session = accepted_session(fixture.document);
        assert_finite_accepted_relation(&session, first, kind.expected_rows());
        assert_finite_accepted_relation(&session, second, kind.expected_rows());
        let redundancy = session.accepted_state().unwrap().accepted_redundancy();
        let first_source = source_id(session.design_document(), first);
        let second_source = source_id(session.design_document(), second);
        assert_eq!(redundancy.fully_redundant_sources().len(), 1, "{kind:?}");
        assert!(
            redundancy.fully_redundant_sources()[0] == first_source
                || redundancy.fully_redundant_sources()[0] == second_source,
            "{kind:?}: {redundancy:?}"
        );
        assert_eq!(
            redundancy.sources_containing_redundant_rows(),
            redundancy.fully_redundant_sources(),
            "{kind:?}"
        );
    }
}

#[test]
fn m71_conflicts_retain_design_but_preserve_the_last_accepted_state_until_suppressed() {
    for kind in RelationKind::ALL {
        let mut fixture = fixture(kind, default_transform());
        fixture.fix_all_points_at_design_positions();
        let definition = fixture.default_definition();
        let mut session = accepted_session(fixture.document);
        let before_accepted = session.accepted_state().unwrap().identity();
        let before_document = session.accepted_state().unwrap().document().clone();

        let rejected = session
            .apply(
                session.design_identity(),
                DocumentEdit::CreateConstraint {
                    label: format!("conflicting {}", kind.label()),
                    definition,
                },
            )
            .unwrap();
        let DocumentCommandEffect::CreatedConstraint(constraint) = rejected.value() else {
            panic!("created relation effect expected")
        };
        assert!(rejected.published_accepted_identity().is_none(), "{kind:?}");
        assert!(session.design_document().constraint(*constraint).is_some());
        assert!(
            session
                .last_attempt()
                .solve_result()
                .unwrap()
                .rejection
                .is_some()
        );
        assert_eq!(
            session.accepted_state().unwrap().identity(),
            before_accepted
        );
        assert_eq!(
            session.accepted_state().unwrap().document(),
            &before_document
        );
        assert!(session.accepted_state_for_current_input().is_none());

        let source = source_id(session.design_document(), *constraint);
        let repaired = session
            .apply(
                session.design_identity(),
                DocumentEdit::SetSourceSuppressed {
                    source,
                    suppressed: true,
                },
            )
            .unwrap();
        assert!(repaired.published_accepted_identity().is_some(), "{kind:?}");
        assert!(session.accepted_state_for_current_input().is_some());

        let rejected_again = session
            .apply(
                session.design_identity(),
                DocumentEdit::SetSourceSuppressed {
                    source,
                    suppressed: false,
                },
            )
            .unwrap();
        assert!(
            rejected_again.published_accepted_identity().is_none(),
            "{kind:?}"
        );
        assert!(!session.design_document().source(source).unwrap().suppressed);
        assert!(
            session
                .accepted_state()
                .unwrap()
                .document()
                .source(source)
                .unwrap()
                .suppressed
        );
    }
}

#[test]
fn m71_invalid_operands_reject_atomically_without_advancing_retained_authority() {
    for kind in RelationKind::ALL {
        let fixture = fixture(kind, default_transform());
        let mut session = accepted_session(fixture.document);
        let before_input = session.prepared_input();
        let before_design = session.design_document().clone();
        let invalid = match kind {
            RelationKind::HorizontalPoints => DocumentConstraintDefinition::HorizontalPoints {
                first: fixture.points[0],
                second: fixture.points[0],
            },
            RelationKind::VerticalPoints => DocumentConstraintDefinition::VerticalPoints {
                first: fixture.points[0],
                second: fixture.points[0],
            },
            RelationKind::Concentric => DocumentConstraintDefinition::Concentric {
                first: DocumentCenterRef {
                    curve: fixture.curves[0],
                },
                second: DocumentCenterRef {
                    curve: fixture.curves[0],
                },
            },
            RelationKind::Collinear => {
                let support = DocumentLineSupportRef {
                    span: CurveSpan::line(fixture.curves[0]),
                    direction: DocumentDirectionSense::Forward,
                };
                DocumentConstraintDefinition::Collinear {
                    first: support,
                    second: support,
                }
            }
        };
        assert!(
            session
                .apply(
                    session.design_identity(),
                    DocumentEdit::CreateConstraint {
                        label: "invalid M71 relation".into(),
                        definition: invalid,
                    },
                )
                .is_err(),
            "{kind:?}"
        );
        assert_eq!(session.prepared_input(), before_input, "{kind:?}");
        assert_eq!(session.design_document(), &before_design, "{kind:?}");
    }
}

#[test]
fn m71_suppression_reactivation_and_accepted_history_keep_exact_relation_identity() {
    for kind in RelationKind::ALL {
        let fixture = fixture(kind, default_transform());
        let definition = fixture.default_definition();
        let mut session = SketchDocumentSession::new(
            fixture.document,
            DocumentSolveRequest::default().without_previous_state_preferences(),
            SolverConfig::default(),
        )
        .unwrap();
        let created = session
            .apply(DocumentCommand::new(
                session.revision(),
                DocumentEdit::CreateConstraint {
                    label: kind.label().into(),
                    definition,
                },
            ))
            .unwrap();
        let Some(DocumentCommandEffect::CreatedConstraint(constraint)) = created.effect else {
            panic!("created relation effect expected")
        };
        let source = source_id(session.document(), constraint);
        assert_eq!(
            runtime_row_count(session.document(), constraint),
            Some(kind.expected_rows())
        );

        session
            .apply(DocumentCommand::new(
                session.revision(),
                DocumentEdit::SetSourceSuppressed {
                    source,
                    suppressed: true,
                },
            ))
            .unwrap();
        assert!(session.document().source(source).unwrap().suppressed);
        assert_eq!(runtime_row_count(session.document(), constraint), None);

        session.undo(session.revision()).unwrap();
        assert!(!session.document().source(source).unwrap().suppressed);
        assert_eq!(
            runtime_row_count(session.document(), constraint),
            Some(kind.expected_rows())
        );
        session.redo(session.revision()).unwrap();
        assert!(session.document().source(source).unwrap().suppressed);
        assert_eq!(runtime_row_count(session.document(), constraint), None);

        session
            .apply(DocumentCommand::new(
                session.revision(),
                DocumentEdit::SetSourceSuppressed {
                    source,
                    suppressed: false,
                },
            ))
            .unwrap();
        assert_eq!(
            runtime_row_count(session.document(), constraint),
            Some(kind.expected_rows())
        );
    }
}

#[test]
fn m71_parent_point_edits_preserve_relation_identity_and_recover_valid_geometry() {
    for kind in RelationKind::ALL {
        let mut fixture = fixture(kind, default_transform());
        let constraint = fixture.add_relation();
        let source = source_id(&fixture.document, constraint);
        let moved_point = fixture.points[0];
        let original = fixture.document.point(moved_point).unwrap().position;
        let target = [original[0] + 0.35, original[1] - 0.27];
        let mut session = accepted_session(fixture.document);
        let before_design = session.design_identity();
        let before_accepted = session.accepted_state().unwrap().identity();

        let outcome = session
            .apply(
                before_design,
                DocumentEdit::SetPointPosition {
                    point: moved_point,
                    position: target,
                },
            )
            .unwrap();
        assert_eq!(
            outcome.value(),
            &DocumentCommandEffect::UpdatedPoint(moved_point),
            "{kind:?}"
        );
        assert!(outcome.published_accepted_identity().is_some(), "{kind:?}");
        assert_ne!(session.design_identity(), before_design, "{kind:?}");
        assert_ne!(
            session.accepted_state().unwrap().identity(),
            before_accepted,
            "{kind:?}"
        );
        assert_eq!(
            session
                .design_document()
                .point(moved_point)
                .unwrap()
                .position
                .map(f64::to_bits),
            target.map(f64::to_bits),
            "{kind:?} retained parent intent"
        );
        assert_eq!(
            source_id(session.design_document(), constraint),
            source,
            "{kind:?} source identity"
        );
        assert_eq!(
            runtime_row_count(session.design_document(), constraint),
            Some(kind.expected_rows()),
            "{kind:?} runtime mapping"
        );
        assert_finite_accepted_relation(&session, constraint, kind.expected_rows());
        let positions = accepted_positions(&session, &fixture.points);
        assert_relation_geometry(kind, &positions, 1.0);
    }
}

#[test]
fn m71_dependency_deletion_is_conservative_and_cascade_closes_over_relations() {
    for kind in RelationKind::ALL {
        let mut fixture = fixture(kind, default_transform());
        let constraint = fixture.add_relation();
        let dependency = fixture.first_dependency();
        let mut conservative = fixture.document.clone();
        assert!(
            matches!(
                conservative.remove(dependency),
                Err(DocumentError::ObjectInUse(_))
            ),
            "{kind:?}"
        );
        assert!(conservative.constraint(constraint).is_some());

        let mut cascade = fixture.document.clone();
        cascade.remove_many_with_dependents(&[dependency]).unwrap();
        assert!(cascade.constraint(constraint).is_none(), "{kind:?}");
        assert!(!contains_object(&cascade, dependency), "{kind:?}");

        let mut relation_only = fixture.document;
        relation_only
            .remove_with_owned_state(DocumentObjectId::Constraint(constraint))
            .unwrap();
        assert!(relation_only.constraint(constraint).is_none());
        assert!(contains_object(&relation_only, dependency));
        accepted_session(relation_only);
    }
}

#[test]
fn m71_prepared_relation_work_is_non_mutating_until_exact_cas_and_stale_patches_fail_closed() {
    for kind in RelationKind::ALL {
        let fixture = fixture(kind, default_transform());
        let first_definition = fixture.default_definition();
        let second_definition = fixture.definition(
            true,
            DocumentDirectionSense::Reverse,
            DocumentDirectionSense::Forward,
        );
        let mut session = accepted_session(fixture.document);
        let before = session.prepared_input();
        let snapshot = session.prepared_snapshot();
        let first = snapshot.clone().prepare(PreparedSketchOperation::Apply(
            DocumentEdit::CreateConstraint {
                label: format!("prepared {}", kind.label()),
                definition: first_definition,
            },
        ));
        let second = snapshot.prepare(PreparedSketchOperation::Apply(
            DocumentEdit::CreateConstraint {
                label: format!("stale prepared {}", kind.label()),
                definition: second_definition,
            },
        ));
        let first = completed_patch(
            first
                .execute(geosolve_core::OperationControl::unlimited())
                .unwrap(),
        );
        let second = completed_patch(
            second
                .execute(geosolve_core::OperationControl::unlimited())
                .unwrap(),
        );
        assert_eq!(session.prepared_input(), before, "{kind:?}");
        assert!(session.design_document().constraints().is_empty());

        let proposed = first.proposed_commit();
        assert!(proposed.accepted_state_identity().is_some());
        assert_eq!(session.commit_prepared_patch(first).unwrap(), proposed);
        assert_eq!(session.design_document().constraints().len(), 1);
        let constraint = session.design_document().constraints()[0].id;
        assert_eq!(
            runtime_row_count(session.design_document(), constraint),
            Some(kind.expected_rows())
        );
        let committed = session.prepared_input();
        assert!(matches!(
            session.commit_prepared_patch(second),
            Err(DocumentSessionError::StalePreparedPatch { .. })
        ));
        assert_eq!(session.prepared_input(), committed, "{kind:?}");
        assert_eq!(session.design_document().constraints().len(), 1);
    }
}
