use std::fmt::Debug;

use num_dual::DualDVec64;

use crate::{
    EvaluationError, LinearizationStorage, LocalJacobian, ResidualEvaluator, VariableValue,
};

pub(crate) enum AdVariableValue {
    Scalar(DualDVec64),
    Vec2([DualDVec64; 2]),
    Pose2([DualDVec64; 3]),
}

pub(crate) trait LocalAdFormulaClone {
    fn clone_box(&self) -> Box<dyn LocalAdFormula>;
}

impl<T> LocalAdFormulaClone for T
where
    T: LocalAdFormula + Clone + 'static,
{
    fn clone_box(&self) -> Box<dyn LocalAdFormula> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn LocalAdFormula> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

pub(crate) trait LocalAdFormula: LocalAdFormulaClone + Debug + Send + Sync {
    fn evaluate(&self, variables: &[AdVariableValue]) -> Result<Vec<DualDVec64>, EvaluationError>;
}

#[derive(Clone, Debug)]
pub(crate) struct LocalAdEvaluator {
    formula: Box<dyn LocalAdFormula>,
}

impl LocalAdEvaluator {
    pub(crate) fn new(formula: impl LocalAdFormula + 'static) -> Self {
        Self {
            formula: Box::new(formula),
        }
    }

    fn evaluate_seeded(
        &self,
        variables: &[VariableValue],
        step_scales: &[Vec<f64>],
    ) -> Result<(Vec<DualDVec64>, Vec<usize>), EvaluationError> {
        if variables.len() != step_scales.len()
            || variables
                .iter()
                .zip(step_scales)
                .any(|(value, scales)| value.kind().tangent_dimension() != scales.len())
        {
            return Err(EvaluationError::invalid_geometry(
                "local AD incidence and tangent scales do not match",
            ));
        }
        let width = step_scales.iter().map(Vec::len).sum();
        let mut offsets = Vec::with_capacity(variables.len());
        let mut offset = 0;
        let mut dual_variables = Vec::with_capacity(variables.len());
        for (value, scales) in variables.iter().zip(step_scales) {
            offsets.push(offset);
            dual_variables.push(retract_normalized_tangent(*value, scales, width, offset));
            offset += scales.len();
        }
        self.formula
            .evaluate(&dual_variables)
            .map(|values| (values, offsets))
    }
}

impl ResidualEvaluator for LocalAdEvaluator {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let scales = variables
            .iter()
            .map(|value| vec![1.0; value.kind().tangent_dimension()])
            .collect::<Vec<_>>();
        let (values, _) = self.evaluate_seeded(variables, &scales)?;
        Ok(values.into_iter().map(|value| value.re).collect())
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let scales = variables
            .iter()
            .map(|value| vec![1.0; value.kind().tangent_dimension()])
            .collect::<Vec<_>>();
        let (values, offsets) = self.evaluate_seeded(variables, &scales)?;
        let rows = values.len();
        Ok(variables
            .iter()
            .enumerate()
            .map(|(block, variable)| {
                let columns = variable.kind().tangent_dimension();
                let mut derivatives = Vec::with_capacity(rows * columns);
                for value in &values {
                    for column in 0..columns {
                        derivatives.push(derivative(value, offsets[block] + column));
                    }
                }
                LocalJacobian::new(rows, columns, derivatives)
            })
            .collect())
    }

    fn linearize(
        &self,
        variables: &[VariableValue],
        storage: &mut LinearizationStorage<'_, '_>,
    ) -> Option<Result<(), EvaluationError>> {
        Some((|| {
            if variables.len() != storage.jacobian_block_count() {
                return Err(EvaluationError::invalid_geometry(
                    "local AD incidence does not match fused storage",
                ));
            }
            let step_scales = (0..storage.jacobian_block_count())
                .map(|block| {
                    storage
                        .jacobian_block(block)
                        .expect("block index was checked")
                        .step_scales()
                        .to_vec()
                })
                .collect::<Vec<_>>();
            let (values, offsets) = self.evaluate_seeded(variables, &step_scales)?;
            if values.len() != storage.residuals().len() {
                return Err(EvaluationError::invalid_geometry(
                    "local AD output does not match fused residual storage",
                ));
            }
            for (target, value) in storage.residuals_mut().iter_mut().zip(&values) {
                *target = value.re;
            }
            for (block, offset) in offsets.iter().copied().enumerate() {
                let output = storage
                    .jacobian_block_mut(block)
                    .expect("block index was checked");
                let columns = output.columns();
                if output.rows() != values.len() || output.step_scales().len() != columns {
                    return Err(EvaluationError::invalid_geometry(
                        "local AD Jacobian shape does not match fused storage",
                    ));
                }
                for (row, value) in values.iter().enumerate() {
                    for column in 0..columns {
                        // AD was seeded with normalized tangent increments, so these
                        // derivatives must never be converted through a raw 1/scale.
                        output.values_mut()[row * columns + column] =
                            derivative(value, offset + column);
                    }
                }
            }
            storage.mark_normalized_tangent_jacobians();
            Ok(())
        })())
    }
}

// This is the single M9 retraction seam. M11 replaces the Pose2 branch with
// right-manifold retraction without changing formulas or AD seeding policy.
fn retract_normalized_tangent(
    value: VariableValue,
    step_scales: &[f64],
    width: usize,
    offset: usize,
) -> AdVariableValue {
    let seed = |real: f64, coordinate: usize| {
        let mut dual = DualDVec64::from_re(real).derivative(width, offset + coordinate);
        dual.eps.0.as_mut().expect("seeded derivative")[offset + coordinate] =
            step_scales[coordinate];
        dual
    };
    match value {
        VariableValue::Scalar(value) => AdVariableValue::Scalar(seed(value, 0)),
        VariableValue::Vec2(value) => AdVariableValue::Vec2([seed(value[0], 0), seed(value[1], 1)]),
        VariableValue::Pose2(value) => {
            AdVariableValue::Pose2([seed(value[0], 0), seed(value[1], 1), seed(value[2], 2)])
        }
    }
}

fn derivative(value: &DualDVec64, index: usize) -> f64 {
    value
        .eps
        .0
        .as_ref()
        .map_or(0.0, |derivatives| derivatives[index])
}

#[cfg(test)]
mod tests {
    use num_dual::DualNum;

    use super::*;
    use crate::{
        AuditBinding, Problem, ResidualBlock, ResidualCategory, ResidualRowAudit, SourceConstraint,
        VariableBlock,
    };

    #[derive(Clone, Debug)]
    struct MixedFormula {
        scale: f64,
    }

    impl LocalAdFormula for MixedFormula {
        fn evaluate(
            &self,
            variables: &[AdVariableValue],
        ) -> Result<Vec<DualDVec64>, EvaluationError> {
            let [
                AdVariableValue::Scalar(scalar),
                AdVariableValue::Vec2(vector),
                AdVariableValue::Pose2(pose),
            ] = variables
            else {
                return Err(EvaluationError::invalid_geometry(
                    "mixed AD formula expected Scalar, Vec2, and Pose2",
                ));
            };
            let angle_cosine = pose[2].clone().cos();
            let first = scalar * scalar
                + vector[0].clone() * angle_cosine * self.scale
                + &pose[0] * &pose[1];
            let difference = &vector[1] - &pose[1];
            let second = &difference * &difference
                + (pose[2].clone() + scalar.clone() / self.scale).sin() * (self.scale * self.scale)
                + pose[0].clone() * self.scale;
            Ok(vec![first, second])
        }
    }

    #[derive(Clone, Debug)]
    struct MixedAnalytic {
        scale: f64,
    }

    impl ResidualEvaluator for MixedAnalytic {
        fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
            let [
                VariableValue::Scalar(scalar),
                VariableValue::Vec2(vector),
                VariableValue::Pose2(pose),
            ] = variables
            else {
                return Err(EvaluationError::invalid_geometry(
                    "mixed analytic formula expected Scalar, Vec2, and Pose2",
                ));
            };
            let difference = vector[1] - pose[1];
            Ok(vec![
                scalar * scalar + self.scale * vector[0] * pose[2].cos() + pose[0] * pose[1],
                difference * difference
                    + self.scale * self.scale * (pose[2] + scalar / self.scale).sin()
                    + self.scale * pose[0],
            ])
        }

        fn jacobian(
            &self,
            variables: &[VariableValue],
        ) -> Result<Vec<LocalJacobian>, EvaluationError> {
            let [
                VariableValue::Scalar(scalar),
                VariableValue::Vec2(vector),
                VariableValue::Pose2(pose),
            ] = variables
            else {
                return Err(EvaluationError::invalid_geometry(
                    "mixed analytic formula expected Scalar, Vec2, and Pose2",
                ));
            };
            let difference = vector[1] - pose[1];
            let coupled_cosine = (pose[2] + scalar / self.scale).cos();
            Ok(vec![
                LocalJacobian::new(2, 1, vec![2.0 * scalar, self.scale * coupled_cosine]),
                LocalJacobian::new(
                    2,
                    2,
                    vec![self.scale * pose[2].cos(), 0.0, 0.0, 2.0 * difference],
                ),
                LocalJacobian::new(
                    2,
                    3,
                    vec![
                        pose[1],
                        pose[0],
                        -self.scale * vector[0] * pose[2].sin(),
                        self.scale,
                        -2.0 * difference,
                        self.scale * self.scale * coupled_cosine,
                    ],
                ),
            ])
        }
    }

    fn row(name: &str) -> ResidualRowAudit {
        ResidualRowAudit::new(
            name,
            vec![AuditBinding::new("variables", "mixed AD fixture")],
            "scale squared",
        )
    }

    fn mixed_problem(scale: f64, ad: bool) -> Problem {
        let mut problem = Problem::new();
        let scalar = problem.add_variable(VariableBlock::scalar(0.4 * scale, scale).unwrap());
        let vector = problem.add_variable(
            VariableBlock::vec2([0.7 * scale, -0.2 * scale], [scale, scale]).unwrap(),
        );
        let pose = problem.add_variable(
            VariableBlock::pose2([0.3 * scale, -0.6 * scale, 0.35], [scale, scale, 1.0]).unwrap(),
        );
        let source = problem.add_source(SourceConstraint::new("mixed local AD").unwrap());
        let evaluator: Box<dyn ResidualEvaluator> = if ad {
            Box::new(LocalAdEvaluator::new(MixedFormula { scale }))
        } else {
            Box::new(MixedAnalytic { scale })
        };
        problem
            .add_residual(
                ResidualBlock::new(
                    source,
                    ResidualCategory::Hard,
                    vec![scalar, vector, pose],
                    2,
                    vec![scale * scale; 2],
                    vec![row("mixed row zero"), row("mixed row one")],
                    evaluator,
                )
                .unwrap(),
            )
            .unwrap();
        problem
    }

    impl ResidualEvaluator for Box<dyn ResidualEvaluator> {
        fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
            self.as_ref().evaluate(variables)
        }

        fn jacobian(
            &self,
            variables: &[VariableValue],
        ) -> Result<Vec<LocalJacobian>, EvaluationError> {
            self.as_ref().jacobian(variables)
        }

        fn linearize(
            &self,
            variables: &[VariableValue],
            storage: &mut LinearizationStorage<'_, '_>,
        ) -> Option<Result<(), EvaluationError>> {
            self.as_ref().linearize(variables, storage)
        }
    }

    #[test]
    fn mixed_local_ad_matches_analytic_and_central_difference_at_all_scales() {
        for scale in [1.0e-6, 1.0, 1.0e6] {
            let ad = mixed_problem(scale, true);
            let analytic = mixed_problem(scale, false);
            let ad_dense = ad.assemble_dense().unwrap();
            let analytic_dense = analytic.assemble_dense().unwrap();
            assert_eq!(ad_dense.residuals().len(), analytic_dense.residuals().len());
            for (actual, expected) in ad_dense.residuals().iter().zip(analytic_dense.residuals()) {
                assert!((actual - expected).abs() <= 2.0e-14, "scale={scale:e}");
            }
            for (actual, expected) in ad_dense.jacobian().iter().zip(analytic_dense.jacobian()) {
                assert!((actual - expected).abs() <= 2.0e-14, "scale={scale:e}");
            }
            let ad_fd = ad.check_jacobians(1.0e-6).unwrap();
            let analytic_fd = analytic.check_jacobians(1.0e-6).unwrap();
            assert!(ad_fd.all_within(1.0e-6), "scale={scale:e}: {ad_fd:#?}");
            assert!(
                analytic_fd.all_within(1.0e-6),
                "scale={scale:e}: {analytic_fd:#?}"
            );
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct TinyScaleAtan {
        scale: f64,
    }

    impl LocalAdFormula for TinyScaleAtan {
        fn evaluate(
            &self,
            variables: &[AdVariableValue],
        ) -> Result<Vec<DualDVec64>, EvaluationError> {
            let [AdVariableValue::Scalar(value)] = variables else {
                return Err(EvaluationError::invalid_geometry(
                    "tiny-scale AD formula expected one scalar",
                ));
            };
            Ok(vec![(value.clone() / self.scale).atan()])
        }
    }

    #[test]
    fn normalized_ad_derivative_does_not_require_nonfinite_raw_intermediate() {
        let scale = 1.0e-310;
        let mut problem = Problem::new();
        let variable = problem.add_variable(VariableBlock::scalar(0.0, scale).unwrap());
        let source = problem.add_source(SourceConstraint::new("tiny normalized AD").unwrap());
        problem
            .add_residual(
                ResidualBlock::new(
                    source,
                    ResidualCategory::Hard,
                    vec![variable],
                    1,
                    vec![1.0],
                    vec![row("atan(x / scale)")],
                    LocalAdEvaluator::new(TinyScaleAtan { scale }),
                )
                .unwrap(),
            )
            .unwrap();

        let dense = problem.assemble_dense().unwrap();
        assert_eq!(dense.jacobian().nrows(), 1);
        assert_eq!(dense.jacobian().ncols(), 1);
        assert_eq!(dense.jacobian()[(0, 0)].to_bits(), 1.0_f64.to_bits());
        let finite_difference = problem.check_jacobians(1.0e-6).unwrap();
        assert!(
            finite_difference.all_within(1.0e-6),
            "{finite_difference:#?}"
        );
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum FormulaBranch {
        Positive,
        Negative,
    }

    impl FormulaBranch {
        const fn multiplier(self) -> f64 {
            match self {
                Self::Positive => 1.0,
                Self::Negative => -1.0,
            }
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct BranchedScalarTarget {
        target: f64,
        branch: FormulaBranch,
    }

    impl LocalAdFormula for BranchedScalarTarget {
        fn evaluate(
            &self,
            variables: &[AdVariableValue],
        ) -> Result<Vec<DualDVec64>, EvaluationError> {
            let [AdVariableValue::Scalar(value)] = variables else {
                return Err(EvaluationError::invalid_geometry(
                    "branched AD formula expected one scalar",
                ));
            };
            Ok(vec![value.clone() * self.branch.multiplier() - self.target])
        }
    }

    fn branched_target_problem(value: f64, target: f64, branch: FormulaBranch) -> Problem {
        let mut problem = Problem::new();
        let variable = problem.add_variable(VariableBlock::scalar(value, 1.0).unwrap());
        let source = problem.add_source(SourceConstraint::new("branched AD target").unwrap());
        problem
            .add_residual(
                ResidualBlock::new(
                    source,
                    ResidualCategory::Hard,
                    vec![variable],
                    1,
                    vec![1.0],
                    vec![row("branch * x - target")],
                    LocalAdEvaluator::new(BranchedScalarTarget { target, branch }),
                )
                .unwrap(),
            )
            .unwrap();
        problem
    }

    #[test]
    fn local_ad_solves_exact_and_perturbed_states_without_changing_discrete_formula_branch() {
        for (branch, expected) in [
            (FormulaBranch::Positive, 2.0),
            (FormulaBranch::Negative, -2.0),
        ] {
            let mut exact = branched_target_problem(expected, 2.0, branch);
            let exact_report = exact.solve(crate::SolverConfig::default()).unwrap();
            assert_eq!(exact_report.hard_validity, crate::HardValidity::Valid);
            assert_eq!(
                exact_report.accepted_state.ambient()[0].to_bits(),
                expected.to_bits()
            );
            assert!(exact.check_jacobians(1.0e-6).unwrap().all_within(1.0e-6));

            let mut perturbed = branched_target_problem(expected + 0.25, 2.0, branch);
            let recovered = perturbed.solve(crate::SolverConfig::default()).unwrap();
            assert_eq!(recovered.hard_validity, crate::HardValidity::Valid);
            assert!((recovered.accepted_state.ambient()[0] - expected).abs() <= 1.0e-9);
            assert!(
                perturbed
                    .check_jacobians(1.0e-6)
                    .unwrap()
                    .all_within(1.0e-6)
            );
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct PositiveDomain;

    impl LocalAdFormula for PositiveDomain {
        fn evaluate(
            &self,
            variables: &[AdVariableValue],
        ) -> Result<Vec<DualDVec64>, EvaluationError> {
            let [AdVariableValue::Scalar(value)] = variables else {
                return Err(EvaluationError::invalid_geometry(
                    "positive-domain AD formula expected one scalar",
                ));
            };
            if value.re < 0.0 {
                return Err(EvaluationError::out_of_domain(
                    "AD scalar left its positive domain",
                ));
            }
            Ok(vec![value.clone()])
        }
    }

    #[test]
    fn categorized_local_ad_failure_rolls_back_without_losing_category() {
        let mut problem = Problem::new();
        let variable = problem.add_variable(VariableBlock::scalar(-1.0, 1.0).unwrap());
        let source = problem.add_source(SourceConstraint::new("AD domain failure").unwrap());
        problem
            .add_residual(
                ResidualBlock::new(
                    source,
                    ResidualCategory::Hard,
                    vec![variable],
                    1,
                    vec![1.0],
                    vec![row("positive-domain scalar")],
                    LocalAdEvaluator::new(PositiveDomain),
                )
                .unwrap(),
            )
            .unwrap();
        let initial = problem.packed_state().unwrap();

        let report = problem.solve(crate::SolverConfig::default()).unwrap();

        assert_eq!(report.termination, crate::SolveTermination::InvalidGeometry);
        assert_eq!(report.hard_validity, crate::HardValidity::Invalid);
        assert_eq!(report.accepted_state, initial);
        let audit_row = &report.audit.sources[0].rows[0];
        assert_eq!(
            audit_row.evaluation_status,
            crate::AuditEvaluationStatus::Failed
        );
        assert_eq!(
            audit_row.evaluation_error_category,
            Some(crate::EvaluationErrorCategory::OutOfDomain)
        );
    }
}
