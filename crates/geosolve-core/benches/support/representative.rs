use geosolve_core::{
    AuditBinding, CoreError, DenseAssembly, EvaluationError, HardValidity, LinearizationStorage,
    LocalJacobian, Problem, ResidualBlock, ResidualCategory, ResidualEvaluator, ResidualRowAudit,
    SecondaryStatus, SolveReport, SolveTermination, SourceConstraint, VariableBlock, VariableId,
    VariableValue,
};

pub const HARD_RESIDUAL_TOLERANCE: f64 = 1.0e-9;

pub const CASES: [(Family, usize); 6] = [
    (Family::CadLike, 100),
    (Family::CadLike, 1_000),
    (Family::CadLike, 10_000),
    (Family::LinkageLike, 99),
    (Family::LinkageLike, 999),
    (Family::LinkageLike, 9_999),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Family {
    CadLike,
    LinkageLike,
}

impl Family {
    pub const fn label(self) -> &'static str {
        match self {
            Self::CadLike => "cad_like",
            Self::LinkageLike => "linkage_like",
        }
    }

    const fn block_tangent_dimension(self) -> usize {
        match self {
            Self::CadLike => 2,
            Self::LinkageLike => 3,
        }
    }

    const fn maximum_component_blocks(self) -> usize {
        match self {
            Self::CadLike => 10,
            Self::LinkageLike => 11,
        }
    }

    fn edit_delta(self) -> &'static [f64] {
        match self {
            Self::CadLike => &[0.025, -0.015],
            Self::LinkageLike => &[0.02, -0.01, 0.008],
        }
    }
}

#[derive(Clone, Debug)]
pub struct RepresentativeDefinition {
    family: Family,
    tangent_variables: usize,
    components: Vec<ComponentDefinition>,
}

#[derive(Clone, Debug)]
enum ComponentDefinition {
    Cad { points: Vec<[f64; 2]> },
    Linkage { poses: Vec<[f64; 3]> },
}

#[derive(Debug)]
pub struct CompiledWorkload {
    pub problem: Problem,
    pub edit_variable: VariableId,
    family: Family,
}

impl CompiledWorkload {
    pub fn perturb_edit_variable(&mut self) -> Result<(), CoreError> {
        self.problem
            .apply_local_increment(self.edit_variable, self.family.edit_delta())
    }
}

impl RepresentativeDefinition {
    pub fn new(family: Family, tangent_variables: usize) -> Self {
        let block_dimension = family.block_tangent_dimension();
        assert!(tangent_variables > 0);
        assert_eq!(tangent_variables % block_dimension, 0);
        let block_count = tangent_variables / block_dimension;
        let maximum_component_blocks = family.maximum_component_blocks();
        let mut components = Vec::new();
        let mut first_block = 0;
        while first_block < block_count {
            let component_blocks = (block_count - first_block).min(maximum_component_blocks);
            components.push(match family {
                Family::CadLike => ComponentDefinition::Cad {
                    points: cad_witness(components.len(), first_block, component_blocks),
                },
                Family::LinkageLike => ComponentDefinition::Linkage {
                    poses: linkage_witness(components.len(), first_block, component_blocks),
                },
            });
            first_block += component_blocks;
        }
        Self {
            family,
            tangent_variables,
            components,
        }
    }

    pub const fn tangent_variables(&self) -> usize {
        self.tangent_variables
    }

    pub fn variable_blocks(&self) -> usize {
        self.tangent_variables / self.family.block_tangent_dimension()
    }

    pub fn component_count(&self) -> usize {
        self.components.len()
    }

    pub fn compile(&self) -> Result<CompiledWorkload, CoreError> {
        let mut problem = Problem::new();
        let mut edit_variable = None;
        let mut global_block = 0;
        for (component_index, component) in self.components.iter().enumerate() {
            let first = append_component(&mut problem, component, component_index, global_block)?;
            edit_variable.get_or_insert(first);
            global_block += component.block_count();
        }
        Ok(CompiledWorkload {
            problem,
            edit_variable: edit_variable.expect("representative workload is nonempty"),
            family: self.family,
        })
    }

    pub fn compile_component_shards(&self) -> Result<Vec<Problem>, CoreError> {
        let mut shards = Vec::with_capacity(self.components.len());
        let mut global_block = 0;
        for (component_index, component) in self.components.iter().enumerate() {
            let mut problem = Problem::new();
            append_component(&mut problem, component, component_index, global_block)?;
            global_block += component.block_count();
            shards.push(problem);
        }
        Ok(shards)
    }
}

pub fn validate_definition_shape(
    definition: &RepresentativeDefinition,
    family: Family,
    tangent_variables: usize,
) {
    let block_dimension = family.block_tangent_dimension();
    assert_eq!(definition.family, family);
    assert_eq!(definition.tangent_variables(), tangent_variables);
    assert_eq!(
        definition.variable_blocks(),
        tangent_variables / block_dimension
    );
    assert!(definition.component_count() > 0);
}

pub fn validate_compiled_workload(
    definition: &RepresentativeDefinition,
    workload: &CompiledWorkload,
) {
    let layout = workload
        .problem
        .packed_layout()
        .expect("representative compiled layout must be valid");
    assert_eq!(
        layout.tangent_dimension(),
        definition.tangent_variables(),
        "{} compiled tangent dimension",
        definition.family.label()
    );
    assert_eq!(
        workload
            .problem
            .audit_rows()
            .expect("representative audit rows must be valid")
            .len(),
        definition.tangent_variables(),
        "{} compiled hard row count",
        definition.family.label()
    );
}

pub fn validate_assemblies(definition: &RepresentativeDefinition, assemblies: &[DenseAssembly]) {
    assert_eq!(
        assemblies.len(),
        definition.component_count(),
        "{} assembly shard count",
        definition.family.label()
    );
    let tangent_variables = assemblies
        .iter()
        .map(|assembly| assembly.variable_layout().tangent_dimension())
        .sum::<usize>();
    let hard_rows = assemblies
        .iter()
        .map(|assembly| assembly.residuals().len())
        .sum::<usize>();
    assert_eq!(tangent_variables, definition.tangent_variables());
    assert_eq!(hard_rows, definition.tangent_variables());
    assert!(assemblies.iter().all(|assembly| {
        assembly.residuals().iter().all(|value| value.is_finite())
            && assembly.jacobian().iter().all(|value| value.is_finite())
    }));
}

pub fn validate_report(
    definition: &RepresentativeDefinition,
    report: &SolveReport,
    expected_reused_components: usize,
) {
    let label = definition.family.label();
    assert_eq!(report.termination, SolveTermination::Converged, "{label}");
    assert_eq!(
        report.hard_termination,
        SolveTermination::Converged,
        "{label}"
    );
    assert_eq!(report.hard_validity, HardValidity::Valid, "{label}");
    assert_eq!(
        report.temporary_status,
        SecondaryStatus::NotRequested,
        "{label}"
    );
    assert_eq!(
        report.preference_status,
        SecondaryStatus::NotRequested,
        "{label}"
    );
    assert!(report.hard_residuals_validated, "{label}");
    assert!(report.hard_residual_max.is_finite(), "{label}");
    assert!(
        report.hard_residual_max <= HARD_RESIDUAL_TOLERANCE,
        "{label} hard residual {} exceeds {}",
        report.hard_residual_max,
        HARD_RESIDUAL_TOLERANCE
    );
    assert!(report.rank_is_valid, "{label}");
    assert_eq!(report.rank, definition.tangent_variables(), "{label}");
    assert_eq!(report.left_nullity, 0, "{label}");
    assert_eq!(report.right_nullity, 0, "{label}");
    assert_eq!(report.local_degrees_of_freedom, 0, "{label}");
    assert_eq!(
        report.accepted_state.layout().tangent_dimension(),
        definition.tangent_variables(),
        "{label} accepted tangent dimension"
    );
    assert_eq!(
        report.component_solves.len(),
        definition.component_count(),
        "{label} component report count"
    );
    assert_eq!(
        report
            .component_solves
            .iter()
            .filter(|component| component.reused)
            .count(),
        expected_reused_components,
        "{label} reused component count"
    );
    assert!(report.component_solves.iter().all(|component| {
        component.termination == SolveTermination::Converged
            && component.hard_termination == SolveTermination::Converged
            && component.hard_validity == HardValidity::Valid
            && component.hard_residuals_validated
            && component.hard_residual_max.is_finite()
            && component.hard_residual_max <= HARD_RESIDUAL_TOLERANCE
            && component.rank_is_valid
            && component.left_nullity == 0
            && component.right_nullity == 0
            && component.local_degrees_of_freedom == 0
            && component.rank_machine_tolerance.is_finite()
            && component.rank_machine_tolerance > 0.0
            && component.rank_threshold.is_finite()
            && component.sigma_max.is_finite()
    }));
}

impl ComponentDefinition {
    fn block_count(&self) -> usize {
        match self {
            Self::Cad { points } => points.len(),
            Self::Linkage { poses } => poses.len(),
        }
    }
}

fn cad_witness(
    component_index: usize,
    first_block: usize,
    component_blocks: usize,
) -> Vec<[f64; 2]> {
    let component = f64_from_usize(component_index);
    let mut points = Vec::with_capacity(component_blocks);
    points.push([20.0 * component, 2.0 * f64_from_usize(component_index % 7)]);
    for local_index in 1..component_blocks {
        let prior = points[local_index - 1];
        let phase = 0.17 * f64_from_usize(first_block + local_index);
        points.push([prior[0] + 1.0, prior[1] + 0.12 * phase.sin()]);
    }
    points
}

fn linkage_witness(
    component_index: usize,
    first_block: usize,
    component_blocks: usize,
) -> Vec<[f64; 3]> {
    let component = f64_from_usize(component_index);
    let mut poses = Vec::with_capacity(component_blocks);
    poses.push([
        18.0 * component,
        2.5 * f64_from_usize(component_index % 5),
        0.025 * f64_from_usize(component_index % 9),
    ]);
    for local_index in 1..component_blocks {
        let prior = poses[local_index - 1];
        let global_index = first_block + local_index;
        let direction = if global_index.is_multiple_of(2) {
            1.0
        } else {
            -1.0
        };
        let angle = prior[2] + direction * (0.012 + 0.001 * f64_from_usize(global_index % 5));
        poses.push([prior[0] + prior[2].cos(), prior[1] + prior[2].sin(), angle]);
    }
    poses
}

fn append_component(
    problem: &mut Problem,
    component: &ComponentDefinition,
    component_index: usize,
    first_global_block: usize,
) -> Result<VariableId, CoreError> {
    match component {
        ComponentDefinition::Cad { points } => {
            append_cad_component(problem, points, component_index, first_global_block)
        }
        ComponentDefinition::Linkage { poses } => {
            append_linkage_component(problem, poses, component_index, first_global_block)
        }
    }
}

fn append_cad_component(
    problem: &mut Problem,
    points: &[[f64; 2]],
    component_index: usize,
    first_global_block: usize,
) -> Result<VariableId, CoreError> {
    let variables: Vec<_> = points
        .iter()
        .enumerate()
        .map(|(local_index, &point)| {
            let global_index = first_global_block + local_index;
            let phase = f64_from_usize(global_index + 1);
            problem.add_variable(
                VariableBlock::vec2(
                    [
                        point[0] + 0.035 * (0.73 * phase).sin(),
                        point[1] + 0.035 * (0.41 * phase).cos(),
                    ],
                    [1.0, 1.0],
                )
                .expect("benchmark Vec2 is finite"),
            )
        })
        .collect();

    let anchor_source = problem.add_source(SourceConstraint::new(format!(
        "CAD component {component_index} anchor"
    ))?);
    problem.add_residual(ResidualBlock::new(
        anchor_source,
        ResidualCategory::Hard,
        vec![variables[0]],
        2,
        vec![1.0, 1.0],
        vec![
            audit_row("(point.x - anchor.x) / length_scale", "anchor x"),
            audit_row("(point.y - anchor.y) / length_scale", "anchor y"),
        ],
        Vec2Anchor { target: points[0] },
    )?)?;

    for local_index in 1..points.len() {
        let delta = [
            points[local_index][0] - points[local_index - 1][0],
            points[local_index][1] - points[local_index - 1][1],
        ];
        let source = problem.add_source(SourceConstraint::new(format!(
            "CAD component {component_index} edge {local_index}"
        ))?);
        problem.add_residual(ResidualBlock::new(
            source,
            ResidualCategory::Hard,
            vec![variables[local_index - 1], variables[local_index]],
            2,
            vec![1.0, 1.0],
            vec![
                audit_row("(next.x - prior.x - delta.x) / length_scale", "edge x"),
                audit_row("(next.y - prior.y - delta.y) / length_scale", "edge y"),
            ],
            Vec2Edge { delta },
        )?)?;
    }
    Ok(variables[0])
}

fn append_linkage_component(
    problem: &mut Problem,
    poses: &[[f64; 3]],
    component_index: usize,
    first_global_block: usize,
) -> Result<VariableId, CoreError> {
    let variables: Vec<_> = poses
        .iter()
        .enumerate()
        .map(|(local_index, &pose)| {
            let global_index = first_global_block + local_index;
            let phase = f64_from_usize(global_index + 1);
            problem.add_variable(
                VariableBlock::pose2(
                    [
                        pose[0] + 0.025 * (0.61 * phase).sin(),
                        pose[1] + 0.025 * (0.37 * phase).cos(),
                        pose[2] + 0.012 * (0.29 * phase).sin(),
                    ],
                    [1.0, 1.0, 1.0],
                )
                .expect("benchmark Pose2 is finite"),
            )
        })
        .collect();

    let anchor_source = problem.add_source(SourceConstraint::new(format!(
        "linkage component {component_index} anchor"
    ))?);
    problem.add_residual(ResidualBlock::new(
        anchor_source,
        ResidualCategory::Hard,
        vec![variables[0]],
        3,
        vec![1.0, 1.0, 1.0],
        vec![
            audit_row("(body.x - anchor.x) / length_scale", "anchor x"),
            audit_row("(body.y - anchor.y) / length_scale", "anchor y"),
            audit_row("body.angle - anchor.angle", "anchor angle"),
        ],
        Pose2Anchor { target: poses[0] },
    )?)?;

    for local_index in 1..poses.len() {
        let angle_delta = poses[local_index][2] - poses[local_index - 1][2];
        let source = problem.add_source(SourceConstraint::new(format!(
            "linkage component {component_index} weld {local_index}"
        ))?);
        problem.add_residual(ResidualBlock::new(
            source,
            ResidualCategory::Hard,
            vec![variables[local_index - 1], variables[local_index]],
            3,
            vec![1.0, 1.0, 1.0],
            vec![
                audit_row("(first.out.x - second.origin.x) / length_scale", "joint x"),
                audit_row("(first.out.y - second.origin.y) / length_scale", "joint y"),
                audit_row("second.angle - first.angle - angle_delta", "relative angle"),
            ],
            Pose2Weld { angle_delta },
        )?)?;
    }
    Ok(variables[0])
}

fn audit_row(template: &str, binding: &str) -> ResidualRowAudit {
    ResidualRowAudit::new(
        template,
        vec![AuditBinding::new("benchmark feature", binding)],
        "normalized model unit",
    )
}

fn f64_from_usize(value: usize) -> f64 {
    f64::from(u32::try_from(value).expect("representative benchmark index fits u32"))
}

fn finite(values: Vec<f64>) -> Result<Vec<f64>, EvaluationError> {
    if values.iter().all(|value| value.is_finite()) {
        Ok(values)
    } else {
        Err(EvaluationError::invalid_geometry(
            "representative benchmark produced a non-finite value",
        ))
    }
}

#[derive(Clone, Debug)]
struct Vec2Anchor {
    target: [f64; 2],
}

impl ResidualEvaluator for Vec2Anchor {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Vec2(point)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "CAD anchor expected one Vec2",
            ));
        };
        finite(vec![point[0] - self.target[0], point[1] - self.target[1]])
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(2, 2, vec![1.0, 0.0, 0.0, 1.0])])
    }

    fn linearize(
        &self,
        variables: &[VariableValue],
        storage: &mut LinearizationStorage<'_, '_>,
    ) -> Option<Result<(), EvaluationError>> {
        Some((|| {
            let [VariableValue::Vec2(point)] = variables else {
                return Err(EvaluationError::invalid_geometry(
                    "CAD anchor expected one Vec2",
                ));
            };
            storage
                .residuals_mut()
                .copy_from_slice(&[point[0] - self.target[0], point[1] - self.target[1]]);
            storage
                .jacobian_block_mut(0)
                .expect("anchor incidence")
                .values_mut()
                .copy_from_slice(&[1.0, 0.0, 0.0, 1.0]);
            Ok(())
        })())
    }
}

#[derive(Clone, Debug)]
struct Vec2Edge {
    delta: [f64; 2],
}

impl ResidualEvaluator for Vec2Edge {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Vec2(prior), VariableValue::Vec2(next)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "CAD edge expected two Vec2 values",
            ));
        };
        finite(vec![
            next[0] - prior[0] - self.delta[0],
            next[1] - prior[1] - self.delta[1],
        ])
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![
            LocalJacobian::new(2, 2, vec![-1.0, 0.0, 0.0, -1.0]),
            LocalJacobian::new(2, 2, vec![1.0, 0.0, 0.0, 1.0]),
        ])
    }

    fn linearize(
        &self,
        variables: &[VariableValue],
        storage: &mut LinearizationStorage<'_, '_>,
    ) -> Option<Result<(), EvaluationError>> {
        Some((|| {
            let [VariableValue::Vec2(prior), VariableValue::Vec2(next)] = variables else {
                return Err(EvaluationError::invalid_geometry(
                    "CAD edge expected two Vec2 values",
                ));
            };
            storage.residuals_mut().copy_from_slice(&[
                next[0] - prior[0] - self.delta[0],
                next[1] - prior[1] - self.delta[1],
            ]);
            storage
                .jacobian_block_mut(0)
                .expect("prior incidence")
                .values_mut()
                .copy_from_slice(&[-1.0, 0.0, 0.0, -1.0]);
            storage
                .jacobian_block_mut(1)
                .expect("next incidence")
                .values_mut()
                .copy_from_slice(&[1.0, 0.0, 0.0, 1.0]);
            Ok(())
        })())
    }
}

#[derive(Clone, Debug)]
struct Pose2Anchor {
    target: [f64; 3],
}

impl ResidualEvaluator for Pose2Anchor {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Pose2(pose)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "linkage anchor expected one Pose2",
            ));
        };
        finite(vec![
            pose[0] - self.target[0],
            pose[1] - self.target[1],
            pose[2] - self.target[2],
        ])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [VariableValue::Pose2(pose)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "linkage anchor expected one Pose2",
            ));
        };
        let (sine, cosine) = pose[2].sin_cos();
        Ok(vec![LocalJacobian::new(
            3,
            3,
            vec![cosine, -sine, 0.0, sine, cosine, 0.0, 0.0, 0.0, 1.0],
        )])
    }

    fn linearize(
        &self,
        variables: &[VariableValue],
        storage: &mut LinearizationStorage<'_, '_>,
    ) -> Option<Result<(), EvaluationError>> {
        Some((|| {
            let [VariableValue::Pose2(pose)] = variables else {
                return Err(EvaluationError::invalid_geometry(
                    "linkage anchor expected one Pose2",
                ));
            };
            let (sine, cosine) = pose[2].sin_cos();
            storage.residuals_mut().copy_from_slice(&[
                pose[0] - self.target[0],
                pose[1] - self.target[1],
                pose[2] - self.target[2],
            ]);
            storage
                .jacobian_block_mut(0)
                .expect("anchor incidence")
                .values_mut()
                .copy_from_slice(&[cosine, -sine, 0.0, sine, cosine, 0.0, 0.0, 0.0, 1.0]);
            Ok(())
        })())
    }
}

#[derive(Clone, Debug)]
struct Pose2Weld {
    angle_delta: f64,
}

impl ResidualEvaluator for Pose2Weld {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Pose2(first), VariableValue::Pose2(second)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "linkage weld expected two Pose2 values",
            ));
        };
        finite(vec![
            first[0] + first[2].cos() - second[0],
            first[1] + first[2].sin() - second[1],
            second[2] - first[2] - self.angle_delta,
        ])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [VariableValue::Pose2(first), VariableValue::Pose2(second)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "linkage weld expected two Pose2 values",
            ));
        };
        let (first_sine, first_cosine) = first[2].sin_cos();
        let (second_sine, second_cosine) = second[2].sin_cos();
        Ok(vec![
            LocalJacobian::new(
                3,
                3,
                vec![
                    first_cosine,
                    -first_sine,
                    -first_sine,
                    first_sine,
                    first_cosine,
                    first_cosine,
                    0.0,
                    0.0,
                    -1.0,
                ],
            ),
            LocalJacobian::new(
                3,
                3,
                vec![
                    -second_cosine,
                    second_sine,
                    0.0,
                    -second_sine,
                    -second_cosine,
                    0.0,
                    0.0,
                    0.0,
                    1.0,
                ],
            ),
        ])
    }

    fn linearize(
        &self,
        variables: &[VariableValue],
        storage: &mut LinearizationStorage<'_, '_>,
    ) -> Option<Result<(), EvaluationError>> {
        Some((|| {
            let [VariableValue::Pose2(first), VariableValue::Pose2(second)] = variables else {
                return Err(EvaluationError::invalid_geometry(
                    "linkage weld expected two Pose2 values",
                ));
            };
            let (first_sine, first_cosine) = first[2].sin_cos();
            let (second_sine, second_cosine) = second[2].sin_cos();
            storage.residuals_mut().copy_from_slice(&[
                first[0] + first_cosine - second[0],
                first[1] + first_sine - second[1],
                second[2] - first[2] - self.angle_delta,
            ]);
            storage
                .jacobian_block_mut(0)
                .expect("first pose incidence")
                .values_mut()
                .copy_from_slice(&[
                    first_cosine,
                    -first_sine,
                    -first_sine,
                    first_sine,
                    first_cosine,
                    first_cosine,
                    0.0,
                    0.0,
                    -1.0,
                ]);
            storage
                .jacobian_block_mut(1)
                .expect("second pose incidence")
                .values_mut()
                .copy_from_slice(&[
                    -second_cosine,
                    second_sine,
                    0.0,
                    -second_sine,
                    -second_cosine,
                    0.0,
                    0.0,
                    0.0,
                    1.0,
                ]);
            Ok(())
        })())
    }
}
