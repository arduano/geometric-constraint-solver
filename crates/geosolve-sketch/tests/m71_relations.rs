// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::{
    AuditEvaluationStatus, OperationOutcome, ResidualCategory, ResidualId, SolverConfig,
    VariableValue,
};
use geosolve_sketch::{
    CompiledSketch, CurveDefinition, CurveSpan, DesignPointId, DocumentArcSweep, DocumentCenterRef,
    DocumentCommand, DocumentCommandEffect, DocumentConstraintDefinition, DocumentConstraintId,
    DocumentDirectionSense, DocumentEdit, DocumentElementId, DocumentError,
    DocumentHyperbolaBranch, DocumentLineSupportRef, DocumentObjectId, DocumentSessionError,
    DocumentSolveRequest, DocumentSourceId, PersistentId, PreparedSketchOperation,
    PreparedSketchPatch, RetainedSketchDocumentSession, RuntimeSource, ScalarDomain, ScalarUnit,
    SketchDocument, SketchDocumentSession, SketchSolveRequest, SketchSource,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AxisMidpointKind {
    Horizontal,
    Vertical,
}

impl AxisMidpointKind {
    const ALL: [Self; 2] = [Self::Horizontal, Self::Vertical];

    const fn coordinate(self) -> usize {
        match self {
            Self::Horizontal => 1,
            Self::Vertical => 0,
        }
    }

    const fn coordinate_name(self) -> &'static str {
        match self {
            Self::Horizontal => "y",
            Self::Vertical => "x",
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal point to midpoint",
            Self::Vertical => "vertical point to midpoint",
        }
    }

    const fn definition(
        self,
        point: DesignPointId,
        line: CurveSpan,
    ) -> DocumentConstraintDefinition {
        match self {
            Self::Horizontal => {
                DocumentConstraintDefinition::HorizontalPointToMidpoint { point, line }
            }
            Self::Vertical => DocumentConstraintDefinition::VerticalPointToMidpoint { point, line },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AxisMidpointSpanFamily {
    Line,
    Polyline,
}

impl AxisMidpointSpanFamily {
    const ALL: [Self; 2] = [Self::Line, Self::Polyline];
}

struct AxisMidpointFixture {
    document: SketchDocument,
    point: DesignPointId,
    start: DesignPointId,
    end: DesignPointId,
    span: CurveSpan,
    runtime_segment_label: &'static str,
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

fn scaled_position(position: [f64; 2], scale: f64, translation: [f64; 2]) -> [f64; 2] {
    [
        position[0] * scale + translation[0],
        position[1] * scale + translation[1],
    ]
}

fn unit_direction(start: [f64; 2], end: [f64; 2]) -> [f64; 2] {
    let delta = [end[0] - start[0], end[1] - start[1]];
    let length = delta[0].hypot(delta[1]);
    assert!(length.is_finite() && length > 0.0);
    [delta[0] / length, delta[1] / length]
}

fn axis_midpoint_fixture(
    family: AxisMidpointSpanFamily,
    scale: f64,
    translation: [f64; 2],
) -> AxisMidpointFixture {
    let mut document = SketchDocument::new(scale).unwrap();
    let point = document
        .add_point(
            "tracked point",
            scaled_position([3.4, -2.3], scale, translation),
        )
        .unwrap();
    let (start, end, span, runtime_segment_label) = match family {
        AxisMidpointSpanFamily::Line => {
            let start = document
                .add_point(
                    "span start",
                    scaled_position([-2.0, -1.0], scale, translation),
                )
                .unwrap();
            let end = document
                .add_point("span end", scaled_position([4.0, 3.0], scale, translation))
                .unwrap();
            let curve = line(&mut document, "support line", start, end);
            (start, end, CurveSpan::line(curve), "support line")
        }
        AxisMidpointSpanFamily::Polyline => {
            let prefix = document
                .add_point(
                    "polyline prefix",
                    scaled_position([-4.0, 2.0], scale, translation),
                )
                .unwrap();
            let start = document
                .add_point(
                    "span start",
                    scaled_position([-1.0, -2.0], scale, translation),
                )
                .unwrap();
            let end = document
                .add_point("span end", scaled_position([5.0, 4.0], scale, translation))
                .unwrap();
            let prefix_position = document.point(prefix).unwrap().position;
            let start_position = document.point(start).unwrap().position;
            let end_position = document.point(end).unwrap().position;
            let curve = document
                .add_curve(
                    "support polyline",
                    CurveDefinition::Polyline {
                        points: vec![prefix, start, end],
                        closed: false,
                        branch_directions: vec![
                            unit_direction(prefix_position, start_position),
                            unit_direction(start_position, end_position),
                        ],
                    },
                )
                .unwrap();
            (
                start,
                end,
                CurveSpan { curve, segment: 1 },
                "support polyline.segment_2",
            )
        }
    };
    AxisMidpointFixture {
        document,
        point,
        start,
        end,
        span,
        runtime_segment_label,
    }
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

fn compiled_axis_midpoint(
    document: &SketchDocument,
    constraint: DocumentConstraintId,
) -> (CompiledSketch, ResidualId) {
    let source = source_id(document, constraint);
    let lowered = document.lower().unwrap();
    let RuntimeSource::Constraint(runtime) = lowered.mappings().runtime_source(source).unwrap()
    else {
        panic!("axis-midpoint relation must lower to a runtime constraint")
    };
    let compiled = lowered
        .sketch()
        .compile(SketchSolveRequest::default().without_previous_state_preferences())
        .unwrap();
    let residual_ids = compiled
        .source_mappings()
        .iter()
        .find(|mapping| mapping.source == SketchSource::Constraint(runtime))
        .unwrap()
        .residual_ids
        .clone();
    assert_eq!(residual_ids.len(), 1);
    (compiled, residual_ids[0])
}

fn axis_midpoint_raw(
    document: &SketchDocument,
    kind: AxisMidpointKind,
    point: DesignPointId,
    start: DesignPointId,
    end: DesignPointId,
) -> f64 {
    let coordinate = kind.coordinate();
    let point = document.point(point).unwrap().position;
    let start = document.point(start).unwrap().position;
    let end = document.point(end).unwrap().position;
    point[coordinate] - 0.5 * (start[coordinate] + end[coordinate])
}

fn assert_axis_midpoint_formula(
    document: &SketchDocument,
    kind: AxisMidpointKind,
    point: DesignPointId,
    start: DesignPointId,
    end: DesignPointId,
) {
    let raw = axis_midpoint_raw(document, kind, point, start, end);
    let normalized = raw / document.model_scale();
    assert!(raw.is_finite(), "{kind:?} raw residual: {raw:e}");
    assert!(
        normalized.is_finite() && normalized.abs() <= HARD_TOLERANCE,
        "{kind:?} independently normalized residual: {normalized:e}"
    );
}

fn assert_accepted_axis_midpoint_audit(
    session: &RetainedSketchDocumentSession,
    constraint: DocumentConstraintId,
    kind: AxisMidpointKind,
    point: DesignPointId,
    start: DesignPointId,
    end: DesignPointId,
) {
    let accepted = session.accepted_state().unwrap();
    let document = accepted.document();
    let raw = axis_midpoint_raw(document, kind, point, start, end);
    let normalized = raw / document.model_scale();
    let source = source_id(document, constraint);
    let RuntimeSource::Constraint(runtime) = accepted.mappings().runtime_source(source).unwrap()
    else {
        panic!("axis-midpoint relation must retain a runtime constraint mapping")
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
    assert_eq!(audit.rows.len(), 1);
    let row = &audit.rows[0];
    assert_eq!(row.raw_residual.to_bits(), raw.to_bits());
    assert_eq!(row.normalized_residual.to_bits(), normalized.to_bits());
    assert_eq!(row.scale.to_bits(), document.model_scale().to_bits());
    assert_axis_midpoint_formula(document, kind, point, start, end);
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

fn assert_retained_edit_rejects_without_advancing(
    session: &mut RetainedSketchDocumentSession,
    edit: DocumentEdit,
) {
    let before_input = session.prepared_input();
    let before_design = session.design_document().clone();
    let before_attempt = session.last_attempt().identity();
    let before_accepted = session
        .accepted_state()
        .map(geosolve_sketch::SketchAcceptedDocumentState::identity);
    assert!(session.apply(session.design_identity(), edit).is_err());
    assert_eq!(session.prepared_input(), before_input);
    assert_eq!(session.design_document(), &before_design);
    assert_eq!(session.last_attempt().identity(), before_attempt);
    assert_eq!(
        session
            .accepted_state()
            .map(geosolve_sketch::SketchAcceptedDocumentState::identity),
        before_accepted
    );
}

#[test]
fn m71_axis_midpoint_relations_have_exact_runtime_audit_and_geometry_across_supported_spans_and_scales()
 {
    for kind in AxisMidpointKind::ALL {
        for family in AxisMidpointSpanFamily::ALL {
            for scale in [1.0e-6, 1.0, 1.0e6] {
                let translation = [19.0 * scale, -31.0 * scale];
                let mut fixture = axis_midpoint_fixture(family, scale, translation);
                let constraint = fixture
                    .document
                    .add_constraint(kind.label(), kind.definition(fixture.point, fixture.span))
                    .unwrap();
                assert_eq!(runtime_row_count(&fixture.document, constraint), Some(1));

                let (compiled, residual_id) = compiled_axis_midpoint(&fixture.document, constraint);
                let residual = compiled.problem().residual(residual_id).unwrap();
                assert_eq!(residual.output_dimension(), 1);
                assert_eq!(residual.category(), ResidualCategory::Hard);
                assert_eq!(residual.incident_variables().len(), 3);
                let rows = compiled.problem().audit_rows().unwrap();
                let row = rows
                    .iter()
                    .find(|row| row.residual_id == residual_id)
                    .unwrap();
                assert_eq!(row.category, ResidualCategory::Hard);
                assert_eq!(row.row_in_block, 0);
                assert_eq!(row.unit, "model-unit");
                assert_eq!(row.scale.to_bits(), scale.to_bits());
                assert_eq!(
                    row.template,
                    format!(
                        "(tracked point.{coordinate} - (span start.{coordinate} + span end.{coordinate})/2) / model_scale",
                        coordinate = kind.coordinate_name()
                    )
                );
                assert_eq!(
                    row.bindings
                        .iter()
                        .map(|binding| (binding.name.as_str(), binding.value.as_str()))
                        .collect::<Vec<_>>(),
                    vec![
                        ("point", "tracked point"),
                        ("segment", fixture.runtime_segment_label),
                        ("start", "span start"),
                        ("end", "span end"),
                    ]
                );

                let snapshot = compiled.problem().audit_snapshot().unwrap();
                let snapshot_row = snapshot
                    .sources
                    .iter()
                    .flat_map(|source| &source.rows)
                    .find(|row| row.residual_id == residual_id)
                    .unwrap();
                assert_eq!(snapshot_row.incident_variables.len(), 3);
                assert!(snapshot_row.incident_variables.iter().all(|variable| {
                    matches!(variable.value, VariableValue::Vec2(values) if values.iter().all(|value| value.is_finite()))
                }));
                let incident_values = snapshot_row
                    .incident_variables
                    .iter()
                    .map(|variable| variable.value)
                    .collect::<Vec<_>>();
                let [
                    VariableValue::Vec2(point),
                    VariableValue::Vec2(start),
                    VariableValue::Vec2(end),
                ] = incident_values.as_slice()
                else {
                    panic!("axis-midpoint incidence must be P/A/B Vec2 variables")
                };
                let coordinate = kind.coordinate();
                let raw = point[coordinate] - 0.5 * (start[coordinate] + end[coordinate]);
                assert_eq!(snapshot_row.raw_residual.to_bits(), raw.to_bits());
                assert_eq!(
                    snapshot_row.normalized_residual.to_bits(),
                    (raw / scale).to_bits()
                );

                let session = accepted_session(fixture.document);
                assert_finite_accepted_relation(&session, constraint, 1);
                assert_accepted_axis_midpoint_audit(
                    &session,
                    constraint,
                    kind,
                    fixture.point,
                    fixture.start,
                    fixture.end,
                );
            }
        }
    }
}

#[test]
fn m71_axis_midpoint_endpoint_aliases_compile_with_deduplicated_incidence() {
    for kind in AxisMidpointKind::ALL {
        for alias_start in [true, false] {
            let mut document = SketchDocument::new(1.0).unwrap();
            let start = document.add_point("alias start", [-2.0, -1.0]).unwrap();
            let end = document.add_point("alias end", [4.0, 3.0]).unwrap();
            let curve = line(&mut document, "alias line", start, end);
            let point = if alias_start { start } else { end };
            let constraint = document
                .add_constraint(kind.label(), kind.definition(point, CurveSpan::line(curve)))
                .unwrap();
            let (compiled, residual_id) = compiled_axis_midpoint(&document, constraint);
            let residual = compiled.problem().residual(residual_id).unwrap();
            assert_eq!(residual.output_dimension(), 1);
            assert_eq!(residual.incident_variables().len(), 2);
            let snapshot = compiled.problem().audit_snapshot().unwrap();
            let row = snapshot
                .sources
                .iter()
                .flat_map(|source| &source.rows)
                .find(|row| row.residual_id == residual_id)
                .unwrap();
            assert_eq!(row.incident_variables.len(), 2);
            assert!(row.incident_variables.iter().all(|variable| {
                variable
                    .value
                    .ambient_values()
                    .iter()
                    .all(|value| value.is_finite())
            }));

            let session = accepted_session(document);
            assert_finite_accepted_relation(&session, constraint, 1);
            assert_accepted_axis_midpoint_audit(&session, constraint, kind, point, start, end);
            let accepted = session.accepted_state().unwrap().document();
            let start_position = accepted.point(start).unwrap().position;
            let end_position = accepted.point(end).unwrap().position;
            let unconstrained_coordinate = 1 - kind.coordinate();
            assert!(
                (end_position[unconstrained_coordinate] - start_position[unconstrained_coordinate])
                    .abs()
                    > HARD_TOLERANCE
            );
        }
    }
}

#[test]
fn m71_axis_midpoint_axes_coexist_and_follow_live_endpoint_edits_without_identity_churn() {
    let mut fixture = axis_midpoint_fixture(AxisMidpointSpanFamily::Line, 1.0, [0.0, 0.0]);
    let horizontal = fixture
        .document
        .add_constraint(
            AxisMidpointKind::Horizontal.label(),
            AxisMidpointKind::Horizontal.definition(fixture.point, fixture.span),
        )
        .unwrap();
    let vertical = fixture
        .document
        .add_constraint(
            AxisMidpointKind::Vertical.label(),
            AxisMidpointKind::Vertical.definition(fixture.point, fixture.span),
        )
        .unwrap();
    let horizontal_source = source_id(&fixture.document, horizontal);
    let vertical_source = source_id(&fixture.document, vertical);
    let mut session = accepted_session(fixture.document);

    for (point, position) in [
        (fixture.start, [-5.0, 2.0]),
        (fixture.end, [7.0, -4.0]),
        (fixture.start, [-1.5, -3.5]),
    ] {
        let outcome = session
            .apply(
                session.design_identity(),
                DocumentEdit::SetPointPosition { point, position },
            )
            .unwrap();
        assert!(outcome.published_accepted_identity().is_some());
        assert_eq!(
            source_id(session.design_document(), horizontal),
            horizontal_source
        );
        assert_eq!(
            source_id(session.design_document(), vertical),
            vertical_source
        );
        assert_eq!(
            runtime_row_count(session.design_document(), horizontal),
            Some(1)
        );
        assert_eq!(
            runtime_row_count(session.design_document(), vertical),
            Some(1)
        );
        assert_accepted_axis_midpoint_audit(
            &session,
            horizontal,
            AxisMidpointKind::Horizontal,
            fixture.point,
            fixture.start,
            fixture.end,
        );
        assert_accepted_axis_midpoint_audit(
            &session,
            vertical,
            AxisMidpointKind::Vertical,
            fixture.point,
            fixture.start,
            fixture.end,
        );
        let accepted = session.accepted_state().unwrap().document();
        let tracked = accepted.point(fixture.point).unwrap().position;
        let start = accepted.point(fixture.start).unwrap().position;
        let end = accepted.point(fixture.end).unwrap().position;
        for coordinate in [0, 1] {
            assert!(
                (tracked[coordinate] - 0.5 * (start[coordinate] + end[coordinate])).abs()
                    <= HARD_TOLERANCE
            );
        }
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn m71_axis_midpoint_suppression_history_and_rejected_conflict_preserve_authority() {
    for kind in AxisMidpointKind::ALL {
        let fixture = axis_midpoint_fixture(AxisMidpointSpanFamily::Line, 1.0, [0.0, 0.0]);
        let definition = kind.definition(fixture.point, fixture.span);
        let mut history = SketchDocumentSession::new(
            fixture.document.clone(),
            DocumentSolveRequest::default().without_previous_state_preferences(),
            SolverConfig::default(),
        )
        .unwrap();
        let created = history
            .apply(DocumentCommand::new(
                history.revision(),
                DocumentEdit::CreateConstraint {
                    label: kind.label().into(),
                    definition: definition.clone(),
                },
            ))
            .unwrap();
        let Some(DocumentCommandEffect::CreatedConstraint(constraint)) = created.effect else {
            panic!("created axis-midpoint constraint expected")
        };
        let source = source_id(history.document(), constraint);
        assert_eq!(runtime_row_count(history.document(), constraint), Some(1));
        history
            .apply(DocumentCommand::new(
                history.revision(),
                DocumentEdit::SetSourceSuppressed {
                    source,
                    suppressed: true,
                },
            ))
            .unwrap();
        assert!(history.document().source(source).unwrap().suppressed);
        assert_eq!(runtime_row_count(history.document(), constraint), None);
        history.undo(history.revision()).unwrap();
        assert!(!history.document().source(source).unwrap().suppressed);
        assert_eq!(runtime_row_count(history.document(), constraint), Some(1));
        history.redo(history.revision()).unwrap();
        assert!(history.document().source(source).unwrap().suppressed);
        assert_eq!(runtime_row_count(history.document(), constraint), None);
        history
            .apply(DocumentCommand::new(
                history.revision(),
                DocumentEdit::SetSourceSuppressed {
                    source,
                    suppressed: false,
                },
            ))
            .unwrap();
        assert_eq!(runtime_row_count(history.document(), constraint), Some(1));
        assert_eq!(source_id(history.document(), constraint), source);

        let mut conflict = fixture.document;
        for point in [fixture.point, fixture.start, fixture.end] {
            let target = conflict.point(point).unwrap().position;
            conflict
                .add_constraint(
                    "fixed conflict witness",
                    DocumentConstraintDefinition::FixedPoint { point, target },
                )
                .unwrap();
        }
        let mut retained = accepted_session(conflict);
        let before_accepted = retained.accepted_state().unwrap().clone();
        let before_input = retained.prepared_input();
        let rejected = retained
            .apply(
                retained.design_identity(),
                DocumentEdit::CreateConstraint {
                    label: format!("conflicting {}", kind.label()),
                    definition,
                },
            )
            .unwrap();
        let DocumentCommandEffect::CreatedConstraint(rejected_constraint) = rejected.value() else {
            panic!("created conflicting axis-midpoint constraint expected")
        };
        assert!(rejected.published_accepted_identity().is_none());
        assert_ne!(retained.prepared_input(), before_input);
        assert!(
            retained
                .last_attempt()
                .solve_result()
                .unwrap()
                .rejection
                .is_some()
        );
        assert_eq!(
            retained.accepted_state().unwrap().identity(),
            before_accepted.identity()
        );
        assert_eq!(
            retained.accepted_state().unwrap().document(),
            before_accepted.document()
        );
        assert_eq!(
            retained
                .accepted_state()
                .unwrap()
                .solve_result()
                .display_audit,
            before_accepted.solve_result().display_audit
        );
        assert!(retained.accepted_state_for_current_input().is_none());

        let rejected_source = source_id(retained.design_document(), *rejected_constraint);
        let repaired = retained
            .apply(
                retained.design_identity(),
                DocumentEdit::SetSourceSuppressed {
                    source: rejected_source,
                    suppressed: true,
                },
            )
            .unwrap();
        assert!(repaired.published_accepted_identity().is_some());
        let repaired_accepted = retained.accepted_state().unwrap().clone();
        let rejected_again = retained
            .apply(
                retained.design_identity(),
                DocumentEdit::SetSourceSuppressed {
                    source: rejected_source,
                    suppressed: false,
                },
            )
            .unwrap();
        assert!(rejected_again.published_accepted_identity().is_none());
        assert_eq!(
            retained.accepted_state().unwrap().identity(),
            repaired_accepted.identity()
        );
        assert!(
            retained
                .accepted_state()
                .unwrap()
                .document()
                .source(rejected_source)
                .unwrap()
                .suppressed
        );
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn m71_axis_midpoint_dependencies_and_invalid_operands_are_transactional() {
    for kind in AxisMidpointKind::ALL {
        let mut fixture = axis_midpoint_fixture(AxisMidpointSpanFamily::Polyline, 1.0, [0.0, 0.0]);
        let constraint = fixture
            .document
            .add_constraint(kind.label(), kind.definition(fixture.point, fixture.span))
            .unwrap();
        let closure = fixture.document.dependency_closure(constraint);
        for dependency in [
            DocumentElementId::Point(fixture.point),
            DocumentElementId::Curve(fixture.span.curve),
            DocumentElementId::Point(fixture.start),
            DocumentElementId::Point(fixture.end),
        ] {
            assert!(closure.contains(&dependency), "{kind:?}: {dependency:?}");
            let object = match dependency {
                DocumentElementId::Point(point) => DocumentObjectId::Point(point),
                DocumentElementId::Curve(curve) => DocumentObjectId::Curve(curve),
                _ => unreachable!(),
            };
            let mut conservative = fixture.document.clone();
            assert!(matches!(
                conservative.remove(object),
                Err(DocumentError::ObjectInUse(_))
            ));
            assert!(conservative.constraint(constraint).is_some());
            let mut cascade = fixture.document.clone();
            cascade.remove_many_with_dependents(&[object]).unwrap();
            assert!(cascade.constraint(constraint).is_none());
            assert!(!contains_object(&cascade, object));
        }
        let mut relation_only = fixture.document;
        relation_only
            .remove_with_owned_state(DocumentObjectId::Constraint(constraint))
            .unwrap();
        assert!(relation_only.constraint(constraint).is_none());
        for dependency in [
            DocumentObjectId::Point(fixture.point),
            DocumentObjectId::Curve(fixture.span.curve),
            DocumentObjectId::Point(fixture.start),
            DocumentObjectId::Point(fixture.end),
        ] {
            assert!(contains_object(&relation_only, dependency));
        }
        accepted_session(relation_only);

        let valid = axis_midpoint_fixture(AxisMidpointSpanFamily::Line, 1.0, [0.0, 0.0]);
        let mut missing_point_session = accepted_session(valid.document.clone());
        let missing = DesignPointId(PersistentId::from_u128(u128::MAX - 1));
        assert_retained_edit_rejects_without_advancing(
            &mut missing_point_session,
            DocumentEdit::CreateConstraint {
                label: "missing point axis midpoint".into(),
                definition: kind.definition(missing, valid.span),
            },
        );

        let mut nonlinear = valid.document.clone();
        let center = nonlinear.add_point("circle center", [8.0, 5.0]).unwrap();
        let circle = circle(&mut nonlinear, "nonlinear circle", center, 2.0);
        let mut nonlinear_session = accepted_session(nonlinear);
        assert_retained_edit_rejects_without_advancing(
            &mut nonlinear_session,
            DocumentEdit::CreateConstraint {
                label: "nonlinear axis midpoint".into(),
                definition: kind.definition(valid.point, CurveSpan::line(circle)),
            },
        );

        let polyline = axis_midpoint_fixture(AxisMidpointSpanFamily::Polyline, 1.0, [0.0, 0.0]);
        let mut invalid_span_session = accepted_session(polyline.document);
        assert_retained_edit_rejects_without_advancing(
            &mut invalid_span_session,
            DocumentEdit::CreateConstraint {
                label: "invalid span axis midpoint".into(),
                definition: kind.definition(
                    polyline.point,
                    CurveSpan {
                        curve: polyline.span.curve,
                        segment: 99,
                    },
                ),
            },
        );

        let mut degenerate = valid.document;
        let degenerate_constraint = degenerate
            .add_constraint(kind.label(), kind.definition(valid.point, valid.span))
            .unwrap();
        let mut degenerate_session = accepted_session(degenerate);
        let degenerate_source =
            source_id(degenerate_session.design_document(), degenerate_constraint);
        let end_position = degenerate_session
            .design_document()
            .point(valid.end)
            .unwrap()
            .position;
        assert_retained_edit_rejects_without_advancing(
            &mut degenerate_session,
            DocumentEdit::SetPointPosition {
                point: valid.start,
                position: end_position,
            },
        );
        assert_eq!(
            source_id(degenerate_session.design_document(), degenerate_constraint),
            degenerate_source
        );
        assert_eq!(
            runtime_row_count(degenerate_session.design_document(), degenerate_constraint),
            Some(1)
        );
        assert_accepted_axis_midpoint_audit(
            &degenerate_session,
            degenerate_constraint,
            kind,
            valid.point,
            valid.start,
            valid.end,
        );
    }
}

#[test]
fn m71_axis_midpoint_prepared_work_is_non_mutating_until_exact_cas() {
    for kind in AxisMidpointKind::ALL {
        let fixture = axis_midpoint_fixture(AxisMidpointSpanFamily::Line, 1.0, [0.0, 0.0]);
        let mut session = accepted_session(fixture.document);
        let before = session.prepared_input();
        let snapshot = session.prepared_snapshot();
        let first = snapshot.clone().prepare(PreparedSketchOperation::Apply(
            DocumentEdit::CreateConstraint {
                label: format!("prepared {}", kind.label()),
                definition: kind.definition(fixture.point, fixture.span),
            },
        ));
        let second = snapshot.prepare(PreparedSketchOperation::Apply(
            DocumentEdit::CreateConstraint {
                label: format!("stale prepared {}", kind.label()),
                definition: kind.definition(fixture.point, fixture.span),
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
        assert_eq!(session.prepared_input(), before);
        assert!(session.design_document().constraints().is_empty());

        let proposed = first.proposed_commit();
        assert!(proposed.accepted_state_identity().is_some());
        assert_eq!(session.commit_prepared_patch(first).unwrap(), proposed);
        assert_eq!(session.design_document().constraints().len(), 1);
        let constraint = session.design_document().constraints()[0].id;
        assert_eq!(
            runtime_row_count(session.design_document(), constraint),
            Some(1)
        );
        let committed = session.prepared_input();
        assert!(matches!(
            session.commit_prepared_patch(second),
            Err(DocumentSessionError::StalePreparedPatch { .. })
        ));
        assert_eq!(session.prepared_input(), committed);
        assert_eq!(session.design_document().constraints().len(), 1);
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
