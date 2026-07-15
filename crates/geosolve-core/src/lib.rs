//! Domain-independent nonlinear constraint-system infrastructure.

use slotmap::new_key_type;

mod analysis;
#[allow(dead_code)]
mod autodiff;
mod error;
mod linearization;
mod problem;
mod residual;
mod solver;
mod variable;

pub use analysis::{
    ComponentStructuralSummary, IncidenceAnalysis, IncidenceComponent, IncidenceEdge,
    StructuralSummary,
};
pub use error::CoreError;
pub use problem::{
    AuditAnnotations, AuditEvaluationStatus, AuditRowDescriptor, AuditRowSnapshot, AuditSnapshot,
    AuditSourceSnapshot, AuditVariableSnapshot, BlockLayout, DenseAssembly, JacobianBlockReport,
    JacobianCheckReport, PackedLayout, PackedState, Problem, ResidualLayout,
};
pub use residual::{
    AuditBinding, EvaluationError, EvaluationErrorCategory, LinearizationStorage, LocalJacobian,
    LocalJacobianStorage, ResidualBlock, ResidualCategory, ResidualEvaluator, ResidualRowAudit,
    SourceConstraint,
};
pub use solver::{
    ComponentSolveReport, HardValidity, PrioritySolveReport, RedundancyKind, RedundantRowCandidate,
    ResidualRowRef, SecondaryStatus, SolveReport, SolveTermination, SolveTrace, SolveTraceRecord,
    SolverConfig,
};
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
