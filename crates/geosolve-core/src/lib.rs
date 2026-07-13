//! Domain-independent nonlinear constraint-system infrastructure.

use slotmap::new_key_type;

mod error;
mod problem;
mod residual;
mod solver;
mod variable;

pub use error::CoreError;
pub use problem::{
    AuditRowDescriptor, AuditRowSnapshot, AuditSnapshot, AuditSourceSnapshot,
    AuditVariableSnapshot, BlockLayout, DenseAssembly, JacobianBlockReport, JacobianCheckReport,
    PackedLayout, PackedState, Problem, ResidualLayout,
};
pub use residual::{
    AuditBinding, EvaluationError, LocalJacobian, ResidualBlock, ResidualCategory,
    ResidualEvaluator, ResidualRowAudit, SourceConstraint,
};
pub use solver::{SolveReport, SolveTermination, SolveTrace, SolveTraceRecord, SolverConfig};
pub use variable::{VariableBlock, VariableKind, VariableValue};

new_key_type! {
    pub struct VariableId;
    pub struct ResidualId;
    pub struct SourceConstraintId;
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
