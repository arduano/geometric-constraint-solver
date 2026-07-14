use geosolve_core::{
    AuditBinding, AuditEvaluationStatus, AuditSnapshot, Problem, ResidualBlock, ResidualCategory,
    ResidualId, ResidualRowAudit, SolveReport, SolveTermination, SolverConfig, SourceConstraint,
    SourceConstraintId, VariableBlock, VariableId, VariableValue,
};
use geosolve_geometry::Point2;

use crate::model::{
    CoordinateAxis, DimensionKind, DimensionMode, PersistentSource, PointId, SegmentId, Sketch,
    SketchConstraintId, SketchConstraintKind, SketchDimensionId, SketchError, validate_model_scale,
    validate_point,
};
use crate::residuals::{
    AxisDifferenceResidual, CoincidentResidual, DistanceResidual, FixedCoordinateResidual,
    PointTargetResidual,
};

/// Temporary point target supplied for one solve only.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DragTarget {
    pub point: PointId,
    pub target: Point2<f64>,
}

/// Per-solve interaction and minimum-motion objectives.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SketchSolveRequest {
    pub drag: Option<DragTarget>,
    pub previous_state_preferences: bool,
}

impl SketchSolveRequest {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            drag: None,
            previous_state_preferences: true,
        }
    }

    #[must_use]
    pub const fn without_previous_state_preferences(mut self) -> Self {
        self.previous_state_preferences = false;
        self
    }

    #[must_use]
    pub const fn with_drag(mut self, point: PointId, target: Point2<f64>) -> Self {
        self.drag = Some(DragTarget { point, target });
        self
    }
}

impl Default for SketchSolveRequest {
    fn default() -> Self {
        Self::new()
    }
}

/// Domain identity corresponding to one deterministic compiled source entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SketchSource {
    Constraint(SketchConstraintId),
    Dimension(SketchDimensionId),
    DragTarget(PointId),
    PreviousState(PointId),
}

/// Exact relationship between a domain source and zero or one core source.
#[derive(Clone, Debug, PartialEq)]
pub struct SketchSourceMapping {
    pub source: SketchSource,
    pub source_label: String,
    /// `None` only for a reference dimension, which intentionally has no equation.
    pub core_source_id: Option<SourceConstraintId>,
    pub residual_ids: Vec<ResidualId>,
}

/// Exact point-to-core-variable relationship in point insertion order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PointVariableMapping {
    pub point_id: PointId,
    pub variable_id: VariableId,
}

/// Read-only compilation seam for audit, incidence, and Jacobian verification.
#[derive(Debug)]
pub struct CompiledSketch {
    problem: Problem,
    point_variables: Vec<PointVariableMapping>,
    source_mappings: Vec<SketchSourceMapping>,
}

impl CompiledSketch {
    #[must_use]
    pub const fn problem(&self) -> &Problem {
        &self.problem
    }

    #[must_use]
    pub fn point_variables(&self) -> &[PointVariableMapping] {
        &self.point_variables
    }

    #[must_use]
    pub fn source_mappings(&self) -> &[SketchSourceMapping] {
        &self.source_mappings
    }

    #[must_use]
    pub fn variable_for_point(&self, point: PointId) -> Option<VariableId> {
        self.point_variables
            .iter()
            .find_map(|mapping| (mapping.point_id == point).then_some(mapping.variable_id))
    }

    fn solved_geometry(&self) -> Result<SketchGeometry, SketchError> {
        let mut points = Vec::with_capacity(self.point_variables.len());
        for mapping in &self.point_variables {
            let variable = self.problem.variable(mapping.variable_id).ok_or(
                geosolve_core::CoreError::UnknownVariable(mapping.variable_id),
            )?;
            let VariableValue::Vec2([x, y]) = variable.value() else {
                return Err(geosolve_core::CoreError::VariableKindMismatch {
                    expected: geosolve_core::VariableKind::Vec2,
                    actual: variable.kind(),
                }
                .into());
            };
            let position = Point2::new(x, y);
            validate_point(position, "solved point")?;
            points.push(SolvedPoint {
                point_id: mapping.point_id,
                position,
            });
        }
        Ok(SketchGeometry { points })
    }
}

/// One solved point in deterministic insertion order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolvedPoint {
    pub point_id: PointId,
    pub position: Point2<f64>,
}

/// Finite geometry returned for display or downstream queries.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SketchGeometry {
    pub points: Vec<SolvedPoint>,
}

impl SketchGeometry {
    #[must_use]
    pub fn point(&self, point: PointId) -> Option<Point2<f64>> {
        self.points
            .iter()
            .find_map(|item| (item.point_id == point).then_some(item.position))
    }
}

/// Value of one equation-free reference dimension after the solve attempt.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReferenceDimensionValue {
    pub dimension_id: SketchDimensionId,
    pub value: f64,
}

/// Why a core-returned state was not committed to the sketch.
#[derive(Clone, Debug, PartialEq)]
pub enum SolveRejection {
    CoreTermination(SolveTermination),
    HardResidual { maximum: f64, tolerance: f64 },
    IndependentValidationFailed(String),
    SegmentBranchFlipped(SegmentId),
}

/// Domain solve outcome. `geometry` is always the geometry retained by the sketch.
#[derive(Debug)]
pub struct SketchSolveResult {
    pub geometry: SketchGeometry,
    /// Audit evaluated at exactly `geometry`, suitable for display.
    pub display_audit: AuditSnapshot,
    pub reference_values: Vec<ReferenceDimensionValue>,
    pub source_mappings: Vec<SketchSourceMapping>,
    /// Report for the attempted solve, including its candidate-state audit on rejection.
    pub core_report: SolveReport,
    pub rejection: Option<SolveRejection>,
    pub acceptance_hard_residual_max: Option<f64>,
}

impl SketchSolveResult {
    #[must_use]
    pub const fn accepted(&self) -> bool {
        self.rejection.is_none()
    }
}

impl Sketch {
    /// Compiles the current accepted geometry and one transient solve request.
    ///
    /// # Errors
    ///
    /// Returns an error for stale IDs, non-finite geometry, invalid scale, or
    /// a rejected core declaration.
    pub fn compile(&self, request: SketchSolveRequest) -> Result<CompiledSketch, SketchError> {
        validate_model_scale(self.model_scale)?;
        validate_request(self, request)?;

        let mut problem = Problem::new();
        let mut point_variables = Vec::new();
        for (point_id, point) in self.points.iter() {
            validate_point(point.position(), "point position")?;
            let variable_id = problem.add_variable(VariableBlock::vec2(
                [point.position().x, point.position().y],
                [self.model_scale, self.model_scale],
            )?);
            point_variables.push(PointVariableMapping {
                point_id,
                variable_id,
            });
        }

        let mut source_mappings = Vec::new();
        for source in &self.source_order {
            match *source {
                PersistentSource::Constraint(constraint_id) => {
                    let Some(constraint) = self.constraints.get(constraint_id) else {
                        continue;
                    };
                    source_mappings.push(compile_constraint(
                        self,
                        &mut problem,
                        &point_variables,
                        constraint_id,
                        constraint,
                    )?);
                }
                PersistentSource::Dimension(dimension_id) => {
                    let Some(dimension) = self.dimensions.get(dimension_id) else {
                        continue;
                    };
                    source_mappings.push(compile_dimension(
                        self,
                        &mut problem,
                        &point_variables,
                        dimension_id,
                        dimension,
                    )?);
                }
            }
        }

        if let Some(drag) = request.drag {
            source_mappings.push(compile_point_target(
                self,
                &mut problem,
                &point_variables,
                SketchSource::DragTarget(drag.point),
                drag.point,
                drag.target,
                ResidualCategory::Temporary,
                format!("temporary drag target for {}", self.point_name(drag.point)?),
            )?);
        }
        if request.previous_state_preferences {
            for (point_id, point) in self.points.iter() {
                source_mappings.push(compile_point_target(
                    self,
                    &mut problem,
                    &point_variables,
                    SketchSource::PreviousState(point_id),
                    point_id,
                    point.position(),
                    ResidualCategory::Preference,
                    format!("previous-state preference for {}", point.label()),
                )?);
            }
        }

        // Fixed declarations synchronize eagerly in core. Restore the domain's
        // retained coordinates so the compile seam and pre-attempt audit use
        // the exact warm-start state; solve synchronization remains trusted.
        for mapping in &point_variables {
            let position = self.point_position(mapping.point_id)?;
            problem.set_variable_value(
                mapping.variable_id,
                VariableValue::Vec2([position.x, position.y]),
            )?;
        }

        Ok(CompiledSketch {
            problem,
            point_variables,
            source_mappings,
        })
    }

    /// Compiles, solves, independently validates, and conditionally commits a request.
    ///
    /// # Errors
    ///
    /// Returns an error when compilation or the core solve cannot be started.
    /// Numerical and geometric solve failures are returned as rejected results.
    pub fn solve(
        &mut self,
        request: SketchSolveRequest,
        config: SolverConfig,
    ) -> Result<SketchSolveResult, SketchError> {
        let mut compiled = self.compile(request)?;
        let mut retained_audit = compiled.problem.audit_snapshot()?;
        let core_report = compiled.problem.solve(config)?;
        let candidate = compiled.solved_geometry()?;
        let mut acceptance_hard_residual_max = None;

        let rejection = if core_report.termination != SolveTermination::Converged {
            Some(SolveRejection::CoreTermination(core_report.termination))
        } else if !core_report.hard_residuals_validated
            || core_report.hard_residual_max > config.normalized_residual_tolerance
        {
            Some(SolveRejection::HardResidual {
                maximum: core_report.hard_residual_max,
                tolerance: config.normalized_residual_tolerance,
            })
        } else {
            match independent_hard_residual_max(&compiled.problem) {
                Ok(maximum) => {
                    acceptance_hard_residual_max = Some(maximum);
                    if maximum > config.normalized_residual_tolerance {
                        Some(SolveRejection::HardResidual {
                            maximum,
                            tolerance: config.normalized_residual_tolerance,
                        })
                    } else {
                        self.first_flipped_segment(&candidate)
                            .map(SolveRejection::SegmentBranchFlipped)
                    }
                }
                Err(error) => Some(SolveRejection::IndependentValidationFailed(
                    error.to_string(),
                )),
            }
        };

        if rejection.is_none() {
            for point in &candidate.points {
                self.set_point_position(point.point_id, point.position)?;
            }
        }
        let display_audit = if rejection.is_none() {
            core_report.audit.clone()
        } else {
            merge_conflicting_annotations(&mut retained_audit, &core_report.audit);
            retained_audit
        };

        Ok(SketchSolveResult {
            geometry: self.geometry(),
            display_audit,
            reference_values: self.reference_values()?,
            source_mappings: compiled.source_mappings,
            core_report,
            rejection,
            acceptance_hard_residual_max,
        })
    }

    #[must_use]
    pub fn geometry(&self) -> SketchGeometry {
        SketchGeometry {
            points: self
                .points
                .iter()
                .map(|(point_id, point)| SolvedPoint {
                    point_id,
                    position: point.position(),
                })
                .collect(),
        }
    }

    fn reference_values(&self) -> Result<Vec<ReferenceDimensionValue>, SketchError> {
        self.dimensions
            .iter()
            .filter(|(_, dimension)| dimension.mode() == DimensionMode::Reference)
            .map(|(dimension_id, dimension)| {
                Ok(ReferenceDimensionValue {
                    dimension_id,
                    value: self.dimension_value(dimension)?,
                })
            })
            .collect()
    }

    fn first_flipped_segment(&self, geometry: &SketchGeometry) -> Option<SegmentId> {
        self.segments.iter().find_map(|(segment_id, segment)| {
            if !self.segment_branch_is_enforced(segment_id) {
                return None;
            }
            let start = geometry.point(segment.start())?;
            let end = geometry.point(segment.end())?;
            (!segment.branch().is_preserved(start, end)).then_some(segment_id)
        })
    }

    fn point_name(&self, point: PointId) -> Result<&str, SketchError> {
        self.points
            .get(point)
            .map(crate::SketchPoint::label)
            .ok_or(SketchError::UnknownPoint(point))
    }
}

fn validate_request(sketch: &Sketch, request: SketchSolveRequest) -> Result<(), SketchError> {
    if let Some(drag) = request.drag {
        sketch.point_position(drag.point)?;
        validate_point(drag.target, "drag target")?;
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn compile_constraint(
    sketch: &Sketch,
    problem: &mut Problem,
    point_variables: &[PointVariableMapping],
    constraint_id: SketchConstraintId,
    constraint: &crate::SketchConstraint,
) -> Result<SketchSourceMapping, SketchError> {
    let scale = sketch.model_scale;
    let (label, residual) = match constraint.kind() {
        SketchConstraintKind::FixedPoint { point, target } => {
            let point_name = sketch.point_name(point)?;
            let label = format!(
                "constraint {}: fixed point {point_name}",
                constraint.ordinal()
            );
            let source_id = problem.add_source(SourceConstraint::new(&label)?);
            let variable = point_variable(point_variables, point)?;
            let rows = vec![
                audit_row(
                    format!("({point_name}.x - target.x) / model_scale"),
                    point_bindings(point_name, target),
                ),
                audit_row(
                    format!("({point_name}.y - target.y) / model_scale"),
                    point_bindings(point_name, target),
                ),
            ];
            let residual = ResidualBlock::fixed_variable(
                source_id,
                variable,
                VariableValue::Vec2([target.x, target.y]),
                vec![scale, scale],
                rows,
            )?;
            let residual_id = problem.add_residual(residual)?;
            problem.declare_fixed_variable(
                variable,
                VariableValue::Vec2([target.x, target.y]),
                residual_id,
            )?;
            return Ok(equation_mapping(
                SketchSource::Constraint(constraint_id),
                label,
                source_id,
                residual_id,
            ));
        }
        SketchConstraintKind::FixedCoordinate {
            point,
            axis,
            target,
        } => {
            let point_name = sketch.point_name(point)?;
            let coordinate = match axis {
                CoordinateAxis::X => 0,
                CoordinateAxis::Y => 1,
            };
            let axis_name = match axis {
                CoordinateAxis::X => "x",
                CoordinateAxis::Y => "y",
            };
            let label = format!(
                "constraint {}: fixed {point_name}.{axis_name}",
                constraint.ordinal()
            );
            let source_id = problem.add_source(SourceConstraint::new(&label)?);
            let residual = ResidualBlock::new(
                source_id,
                ResidualCategory::Hard,
                vec![point_variable(point_variables, point)?],
                1,
                vec![scale],
                vec![audit_row(
                    format!("({point_name}.{axis_name} - target) / model_scale"),
                    vec![
                        AuditBinding::new("point", point_name),
                        AuditBinding::new("coordinate", axis_name),
                        AuditBinding::new("target", target.to_string()),
                    ],
                )],
                FixedCoordinateResidual { coordinate, target },
            )?;
            (label, residual)
        }
        SketchConstraintKind::Coincident { first, second } => {
            let first_name = sketch.point_name(first)?;
            let second_name = sketch.point_name(second)?;
            let label = format!(
                "constraint {}: {first_name} coincident with {second_name}",
                constraint.ordinal()
            );
            let source_id = problem.add_source(SourceConstraint::new(&label)?);
            let bindings = pair_bindings(first_name, second_name);
            let residual = ResidualBlock::new(
                source_id,
                ResidualCategory::Hard,
                vec![
                    point_variable(point_variables, first)?,
                    point_variable(point_variables, second)?,
                ],
                2,
                vec![scale, scale],
                vec![
                    audit_row(
                        format!("({second_name}.x - {first_name}.x) / model_scale"),
                        bindings.clone(),
                    ),
                    audit_row(
                        format!("({second_name}.y - {first_name}.y) / model_scale"),
                        bindings,
                    ),
                ],
                CoincidentResidual,
            )?;
            (label, residual)
        }
        SketchConstraintKind::Horizontal { segment } => compile_axis_constraint(
            sketch,
            problem,
            point_variables,
            constraint,
            segment,
            1,
            "horizontal",
            "y",
        )?,
        SketchConstraintKind::Vertical { segment } => compile_axis_constraint(
            sketch,
            problem,
            point_variables,
            constraint,
            segment,
            0,
            "vertical",
            "x",
        )?,
    };
    let source_id = residual.source();
    let residual_id = problem.add_residual(residual)?;
    Ok(equation_mapping(
        SketchSource::Constraint(constraint_id),
        label,
        source_id,
        residual_id,
    ))
}

#[allow(clippy::too_many_arguments)]
fn compile_axis_constraint(
    sketch: &Sketch,
    problem: &mut Problem,
    point_variables: &[PointVariableMapping],
    constraint: &crate::SketchConstraint,
    segment_id: SegmentId,
    coordinate: usize,
    orientation: &str,
    coordinate_name: &str,
) -> Result<(String, ResidualBlock), SketchError> {
    let segment = sketch
        .segments
        .get(segment_id)
        .ok_or(SketchError::UnknownSegment(segment_id))?;
    let start_name = sketch.point_name(segment.start())?;
    let end_name = sketch.point_name(segment.end())?;
    let label = format!(
        "constraint {}: {} {orientation}",
        constraint.ordinal(),
        segment.label()
    );
    let source_id = problem.add_source(SourceConstraint::new(&label)?);
    let residual = ResidualBlock::new(
        source_id,
        ResidualCategory::Hard,
        vec![
            point_variable(point_variables, segment.start())?,
            point_variable(point_variables, segment.end())?,
        ],
        1,
        vec![sketch.model_scale],
        vec![audit_row(
            format!(
                "({end_name}.{coordinate_name} - {start_name}.{coordinate_name}) / model_scale"
            ),
            vec![
                AuditBinding::new("segment", segment.label()),
                AuditBinding::new("start", start_name),
                AuditBinding::new("end", end_name),
            ],
        )],
        AxisDifferenceResidual { coordinate },
    )?;
    Ok((label, residual))
}

fn compile_dimension(
    sketch: &Sketch,
    problem: &mut Problem,
    point_variables: &[PointVariableMapping],
    dimension_id: SketchDimensionId,
    dimension: &crate::SketchDimension,
) -> Result<SketchSourceMapping, SketchError> {
    let (first, second, target, subject) = match dimension.kind() {
        DimensionKind::PointDistance {
            first,
            second,
            target,
        } => (
            first,
            second,
            target,
            format!(
                "distance {}-{}",
                sketch.point_name(first)?,
                sketch.point_name(second)?
            ),
        ),
        DimensionKind::SegmentLength { segment, target } => {
            let segment_value = sketch
                .segments
                .get(segment)
                .ok_or(SketchError::UnknownSegment(segment))?;
            (
                segment_value.start(),
                segment_value.end(),
                target,
                format!("length {}", segment_value.label()),
            )
        }
    };
    if dimension.mode() == DimensionMode::Reference {
        return Ok(SketchSourceMapping {
            source: SketchSource::Dimension(dimension_id),
            source_label: format!(
                "dimension {}: reference measurement of {subject}",
                dimension.ordinal()
            ),
            core_source_id: None,
            residual_ids: Vec::new(),
        });
    }

    let label = format!(
        "dimension {}: {subject} = {target} (driving)",
        dimension.ordinal()
    );
    let first_name = sketch.point_name(first)?;
    let second_name = sketch.point_name(second)?;
    let source_id = problem.add_source(SourceConstraint::new(&label)?);
    let residual_id = problem.add_residual(ResidualBlock::new(
        source_id,
        ResidualCategory::Hard,
        vec![
            point_variable(point_variables, first)?,
            point_variable(point_variables, second)?,
        ],
        1,
        vec![sketch.model_scale],
        vec![audit_row(
            format!("(distance({first_name}, {second_name}) - target) / model_scale"),
            vec![
                AuditBinding::new("first", first_name),
                AuditBinding::new("second", second_name),
                AuditBinding::new("target", target.to_string()),
            ],
        )],
        DistanceResidual { target },
    )?)?;
    Ok(equation_mapping(
        SketchSource::Dimension(dimension_id),
        label,
        source_id,
        residual_id,
    ))
}

#[allow(clippy::too_many_arguments)]
fn compile_point_target(
    sketch: &Sketch,
    problem: &mut Problem,
    point_variables: &[PointVariableMapping],
    source: SketchSource,
    point: PointId,
    target: Point2<f64>,
    category: ResidualCategory,
    label: String,
) -> Result<SketchSourceMapping, SketchError> {
    let point_name = sketch.point_name(point)?;
    let source_id = problem.add_source(SourceConstraint::new(&label)?);
    let residual_id = problem.add_residual(ResidualBlock::new(
        source_id,
        category,
        vec![point_variable(point_variables, point)?],
        2,
        vec![sketch.model_scale, sketch.model_scale],
        vec![
            audit_row(
                format!("({point_name}.x - target.x) / model_scale"),
                point_bindings(point_name, target),
            ),
            audit_row(
                format!("({point_name}.y - target.y) / model_scale"),
                point_bindings(point_name, target),
            ),
        ],
        PointTargetResidual {
            target: [target.x, target.y],
        },
    )?)?;
    Ok(equation_mapping(source, label, source_id, residual_id))
}

fn equation_mapping(
    source: SketchSource,
    source_label: String,
    core_source_id: SourceConstraintId,
    residual_id: ResidualId,
) -> SketchSourceMapping {
    SketchSourceMapping {
        source,
        source_label,
        core_source_id: Some(core_source_id),
        residual_ids: vec![residual_id],
    }
}

fn point_variable(
    mappings: &[PointVariableMapping],
    point: PointId,
) -> Result<VariableId, SketchError> {
    mappings
        .iter()
        .find_map(|mapping| (mapping.point_id == point).then_some(mapping.variable_id))
        .ok_or(SketchError::UnknownPoint(point))
}

fn audit_row(template: String, bindings: Vec<AuditBinding>) -> ResidualRowAudit {
    ResidualRowAudit::new(template, bindings, "model-unit")
}

fn point_bindings(point_name: &str, target: Point2<f64>) -> Vec<AuditBinding> {
    vec![
        AuditBinding::new("point", point_name),
        AuditBinding::new("target", format!("({}, {})", target.x, target.y)),
    ]
}

fn pair_bindings(first_name: &str, second_name: &str) -> Vec<AuditBinding> {
    vec![
        AuditBinding::new("first", first_name),
        AuditBinding::new("second", second_name),
    ]
}

fn independent_hard_residual_max(problem: &Problem) -> Result<f64, geosolve_core::CoreError> {
    let audit = problem.audit_snapshot()?;
    let mut maximum = 0.0_f64;
    for row in audit
        .sources
        .iter()
        .flat_map(|source| &source.rows)
        .filter(|row| row.category == ResidualCategory::Hard)
    {
        if row.evaluation_status != AuditEvaluationStatus::Evaluated
            || !row.normalized_residual.is_finite()
        {
            return Err(geosolve_core::CoreError::NonFiniteValue {
                context: "sketch independent hard validation",
                index: row.row_in_block,
                value: row.normalized_residual,
            });
        }
        maximum = maximum.max(row.normalized_residual.abs());
    }
    Ok(maximum)
}

fn merge_conflicting_annotations(retained: &mut AuditSnapshot, attempted: &AuditSnapshot) {
    for retained_source in &mut retained.sources {
        let Some(attempted_source) = attempted
            .sources
            .iter()
            .find(|source| source.source_id == retained_source.source_id)
        else {
            continue;
        };
        retained_source.annotations.conflicting |= attempted_source.annotations.conflicting;
        for retained_row in &mut retained_source.rows {
            if attempted_source.rows.iter().any(|row| {
                row.residual_id == retained_row.residual_id
                    && row.row_in_block == retained_row.row_in_block
                    && row.annotations.conflicting
            }) {
                retained_row.annotations.conflicting = true;
            }
        }
    }
}
