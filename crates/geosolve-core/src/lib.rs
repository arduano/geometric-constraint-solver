//! Domain-independent nonlinear constraint-system infrastructure.

use slotmap::new_key_type;

mod analysis;
#[allow(dead_code)]
mod autodiff;
mod bounds;
mod error;
mod linearization;
mod problem;
mod residual;
mod session;
mod solver;
mod variable;

pub use analysis::{
    ComponentStructuralSummary, IncidenceAnalysis, IncidenceComponent, IncidenceEdge,
    StructuralSummary,
};
pub use bounds::{BoundReport, BoundStatus, CoordinateBound, OneSidedMobility};
pub use error::{CoreError, SensitivityError};
pub use linearization::{
    AcceptedHardComponentLinearization, AcceptedHardLinearization, RawTangentBlock, ReducedHardRow,
    ReducedTangentBlock, SensitivitySolution, SensitivityStatus,
};
pub use problem::{
    AuditAnnotations, AuditBoundAnnotation, AuditEvaluationStatus, AuditRowDescriptor,
    AuditRowSnapshot, AuditSnapshot, AuditSourceSnapshot, AuditVariableSnapshot, BlockLayout,
    DenseAssembly, JacobianBlockReport, JacobianCheckReport, PackedLayout, PackedState, Problem,
    ResidualLayout,
};
pub use residual::{
    AuditBinding, EvaluationError, EvaluationErrorCategory, LinearizationStorage, LocalJacobian,
    LocalJacobianStorage, ResidualBlock, ResidualCategory, ResidualEvaluator,
    ResidualEvaluatorClone, ResidualRowAudit, SourceConstraint,
};
pub use session::{
    AcceptedAuditPatch, ComponentDependencyStamp, SessionCoreRejection, SessionDomainRejection,
    SessionError, SessionPatch, SessionRevisions, SessionTransaction, SessionTransactionRejection,
    SolveSession,
};
pub use solver::{
    ComponentSolveReport, DiagnosticBudget, DiagnosticCompleteness, DiagnosticIncompleteReason,
    DiagnosticStatus, DiagnosticWork, HardValidity, PrioritySolveReport, RedundancyKind,
    RedundantRowCandidate, ResidualRowRef, SecondaryStatus, SolveReport, SolveTermination,
    SolveTrace, SolveTraceRecord, SolverConfig,
};
pub use variable::{VariableBlock, VariableKind, VariableValue};

new_key_type! {
    pub struct VariableId;
    pub struct ResidualId;
    pub struct SourceConstraintId;
    pub struct BoundId;
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
