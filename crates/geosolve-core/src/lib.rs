//! Domain-independent nonlinear constraint-system infrastructure.

use slotmap::new_key_type;

mod error;
mod problem;
mod residual;
mod variable;

pub use error::CoreError;
pub use problem::{
    AuditRowDescriptor, BlockLayout, DenseAssembly, JacobianBlockReport, JacobianCheckReport,
    PackedLayout, PackedState, Problem, ResidualLayout,
};
pub use residual::{
    AuditBinding, EvaluationError, LocalJacobian, ResidualBlock, ResidualCategory,
    ResidualEvaluator, ResidualRowAudit, SourceConstraint,
};
pub use variable::{VariableBlock, VariableKind, VariableValue};

new_key_type! {
    pub struct VariableId;
    pub struct ResidualId;
    pub struct SourceConstraintId;
}

/// Why nonlinear iteration stopped. Constraint-system diagnostics are kept
/// separately because a converged solution may still be underconstrained,
/// redundant, or singular.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolveTermination {
    Converged,
    Stalled,
    IterationLimit,
    InvalidGeometry,
    NumericalFailure,
}

/// Numerical and structural facts needed by callers and the demo UI.
#[derive(Clone, Debug, PartialEq)]
pub struct SolveReport {
    pub termination: SolveTermination,
    pub iterations: usize,
    pub hard_residual_max: f64,
    pub hard_residual_l2: f64,
    pub rank: usize,
    pub local_degrees_of_freedom: usize,
    pub is_singular: bool,
    pub conflicting_sources: Vec<SourceConstraintId>,
    pub redundant_sources: Vec<SourceConstraintId>,
}

/// Centralized tolerances and iteration limits. Domain crates must not invent
/// competing convergence thresholds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolverConfig {
    pub normalized_residual_tolerance: f64,
    pub normalized_step_tolerance: f64,
    pub rank_relative_tolerance: f64,
    pub max_iterations: usize,
}

impl Default for SolverConfig {
    fn default() -> Self {
        Self {
            normalized_residual_tolerance: 1.0e-9,
            normalized_step_tolerance: 1.0e-10,
            rank_relative_tolerance: 1.0e-10,
            max_iterations: 80,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_strict_but_finite() {
        let config = SolverConfig::default();
        assert!(config.normalized_residual_tolerance.is_finite());
        assert!(config.normalized_residual_tolerance > 0.0);
        assert!(config.max_iterations > 0);
    }
}
